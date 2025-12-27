use sysinfo::System;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::events::{AppEvent, DownloadPhase, FormatPopupState, Job, JobStatus, SettingsState, StatusCounts, WorkerCommand};

pub struct App {
    pub jobs: Vec<Job>,
    pub selected_index: usize,
    pub input_buffer: String,
    pub input_mode: bool,
    pub show_help: bool,
    pub show_sysinfo: bool,
    pub should_quit: bool,
    pub confirm_quit: bool,
    pub loading_playlists: usize,
    pub spinner_frame: usize,
    pub format_popup: Option<FormatPopupState>,
    pub settings_popup: Option<SettingsState>,
    pub config: Config,
    pub sysinfo: System,
    worker_tx: mpsc::Sender<WorkerCommand>,
}

impl App {
    pub fn new(config: Config, worker_tx: mpsc::Sender<WorkerCommand>) -> Self {
        Self {
            jobs: Vec::new(),
            selected_index: 0,
            input_buffer: String::new(),
            input_mode: true,
            show_help: false,
            show_sysinfo: true,
            should_quit: false,
            confirm_quit: false,
            loading_playlists: 0,
            spinner_frame: 0,
            format_popup: None,
            settings_popup: None,
            config,
            sysinfo: System::new(),
            worker_tx,
        }
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::AddUrl(url) => {
                if !url.trim().is_empty() {
                    let url = url.trim();
                    if is_playlist_url(url) {
                        self.loading_playlists += 1;
                        if self.worker_tx.try_send(WorkerCommand::FetchPlaylist { url: url.to_string() }).is_err() {
                            tracing::warn!("Worker channel full: FetchPlaylist dropped");
                        }
                    } else {
                        let job = Job::new(url);
                        let job_id = job.id;
                        let job_url = job.url.clone();
                        self.jobs.push(job);
                        if self.worker_tx.try_send(WorkerCommand::FetchFormats { job_id, url: job_url }).is_err() {
                            tracing::warn!("Worker channel full: FetchFormats dropped");
                        }
                    }
                }
            }

            AppEvent::ToggleInputMode => {
                self.input_mode = !self.input_mode;
            }

            AppEvent::SelectNext => {
                if !self.jobs.is_empty() {
                    self.selected_index = (self.selected_index + 1) % self.jobs.len();
                }
            }

            AppEvent::SelectPrev => {
                if !self.jobs.is_empty() {
                    self.selected_index = if self.selected_index == 0 {
                        self.jobs.len() - 1
                    } else {
                        self.selected_index - 1
                    };
                }
            }

            AppEvent::OpenFormatPopup => {
                if let Some(job) = self.jobs.get(self.selected_index) {
                    if job.can_select_format() {
                        self.format_popup = Some(FormatPopupState::new(
                            self.selected_index,
                            job.formats.clone(),
                        ));
                    }
                }
            }

            AppEvent::CloseFormatPopup => {
                self.format_popup = None;
            }

            AppEvent::FormatSelectNext => {
                if let Some(popup) = &mut self.format_popup {
                    let filtered_len = popup.filtered_formats().len();
                    if filtered_len > 0 {
                        popup.selected = (popup.selected + 1) % filtered_len;
                        let visible_height = 10; 
                        if popup.selected >= popup.scroll_offset + visible_height {
                            popup.scroll_offset = popup.selected.saturating_sub(visible_height - 1);
                        } else if popup.selected < popup.scroll_offset {
                            popup.scroll_offset = popup.selected;
                        }
                    }
                }
            }

            AppEvent::FormatSelectPrev => {
                if let Some(popup) = &mut self.format_popup {
                    let filtered_len = popup.filtered_formats().len();
                    if filtered_len > 0 {
                        popup.selected = if popup.selected == 0 {
                            filtered_len - 1
                        } else {
                            popup.selected - 1
                        };
                        if popup.selected < popup.scroll_offset {
                            popup.scroll_offset = popup.selected;
                        }
                    }
                }
            }

            AppEvent::ToggleAudioOnly => {
                if let Some(popup) = &mut self.format_popup {
                    popup.audio_only = !popup.audio_only;
                    popup.selected = 0;
                    popup.scroll_offset = 0;
                }
            }

            AppEvent::ToggleApplyToAll => {
                if let Some(popup) = &mut self.format_popup {
                    popup.apply_to_all = !popup.apply_to_all;
                }
            }

            AppEvent::ConfirmFormat => {
                let Some(popup) = self.format_popup.take() else {
                    return;
                };

                let Some(format) = popup.selected_format().cloned() else {
                    return;
                };

                if popup.apply_to_all {
                    for job in &mut self.jobs {
                        if job.can_select_format() {
                            job.selected_format = Some(format.clone());
                            job.status = JobStatus::Queued;
                        }
                    }
                } else if let Some(job) = self.jobs.get_mut(popup.job_index) {
                    job.selected_format = Some(format);
                    job.status = JobStatus::Queued;
                }
            }

            AppEvent::StartDownloads => {
                for job in &self.jobs {
                    if job.status == JobStatus::Queued {
                        if let Some(fmt) = &job.selected_format {
                            if self.worker_tx.try_send(WorkerCommand::StartJob {
                                job_id: job.id,
                                url: job.url.clone(),
                                format_id: fmt.format_id.clone(),
                            }).is_err() {
                                tracing::warn!("Worker channel full: StartJob dropped");
                            }
                        }
                    }
                }
            }

            AppEvent::CancelJob(id) => {
                if self.worker_tx.try_send(WorkerCommand::CancelJob(id)).is_err() {
                    tracing::warn!("Worker channel full: CancelJob dropped");
                }
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.status = JobStatus::Cancelled;
                }
            }

            AppEvent::RemoveJob(id) => {
                self.jobs.retain(|j| j.id != id);
                if self.selected_index >= self.jobs.len() && !self.jobs.is_empty() {
                    self.selected_index = self.jobs.len() - 1;
                }
            }

            AppEvent::ToggleHelp => {
                self.show_help = !self.show_help;
            }

            AppEvent::ToggleSysInfo => {
                self.show_sysinfo = !self.show_sysinfo;
            }

            AppEvent::Quit => {
                if !self.confirm_quit {
                    self.confirm_quit = true;
                } else {
                    let _ = self.worker_tx.try_send(WorkerCommand::Shutdown);
                    self.should_quit = true;
                }
            }

            AppEvent::CancelQuit => {
                self.confirm_quit = false;
            }

            AppEvent::ConfirmQuit => {
                let _ = self.worker_tx.try_send(WorkerCommand::Shutdown);
                self.should_quit = true;
            }

            AppEvent::JobStarted { id } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.status = JobStatus::Downloading {
                        percent: 0.0,
                        speed: "--".into(),
                        eta: "--".into(),
                        phase: DownloadPhase::Video,
                    };
                }
            }

            AppEvent::FormatsReady { id, title, formats } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.title = Some(title);
                    if formats.is_empty() {
                        job.status = JobStatus::Failed("No formats found".into());
                    } else {
                        job.formats = formats.clone();
                        job.status = JobStatus::Ready { formats };
                    }
                }
            }

            AppEvent::JobProgress { id, percent, speed, eta, phase } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.status = JobStatus::Downloading { percent, speed, eta, phase };
                }
            }

            AppEvent::JobCompleted { id, path } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.status = JobStatus::Completed;
                    job.output_path = Some(path);
                }
            }

            AppEvent::JobFailed { id, error } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.status = JobStatus::Failed(error);
                }
            }

            AppEvent::ToggleSettings => {
                if self.settings_popup.is_some() {
                    self.settings_popup = None;
                } else {
                    self.settings_popup = Some(SettingsState::new(
                        self.config.max_concurrent_downloads,
                        self.config.output_dir.clone(),
                    ));
                }
            }

            AppEvent::CloseSettings => {
                self.settings_popup = None;
            }

            AppEvent::SettingsNext => {
                if let Some(ref mut settings) = self.settings_popup {
                    settings.selected_field = (settings.selected_field + 1).min(1);
                    settings.editing_path = false;
                }
            }

            AppEvent::SettingsPrev => {
                if let Some(ref mut settings) = self.settings_popup {
                    settings.selected_field = settings.selected_field.saturating_sub(1);
                    settings.editing_path = false;
                }
            }

            AppEvent::SettingsIncrement => {
                if let Some(ref mut settings) = self.settings_popup {
                    if settings.selected_field == 0 {
                        settings.concurrent_downloads = (settings.concurrent_downloads + 1).min(10);
                    }
                }
            }

            AppEvent::SettingsDecrement => {
                if let Some(ref mut settings) = self.settings_popup {
                    if settings.selected_field == 0 {
                        settings.concurrent_downloads = settings.concurrent_downloads.saturating_sub(1).max(1);
                    }
                }
            }

            AppEvent::SettingsToggleEdit => {
                if let Some(ref mut settings) = self.settings_popup {
                    if settings.selected_field == 1 {
                        settings.editing_path = !settings.editing_path;
                    }
                }
            }

            AppEvent::SettingsCharInput(c) => {
                if let Some(ref mut settings) = self.settings_popup {
                    if settings.editing_path {
                        settings.output_dir.push(c);
                    }
                }
            }

            AppEvent::SettingsBackspace => {
                if let Some(ref mut settings) = self.settings_popup {
                    if settings.editing_path {
                        settings.output_dir.pop();
                    }
                }
            }

            AppEvent::SaveSettings => {
                if let Some(settings) = self.settings_popup.take() {
                    self.config.max_concurrent_downloads = settings.concurrent_downloads;
                    self.config.output_dir = std::path::PathBuf::from(&settings.output_dir);
                    
                    if self.worker_tx.try_send(WorkerCommand::UpdateConcurrent(settings.concurrent_downloads)).is_err() {
                        tracing::warn!("Failed to send UpdateConcurrent command");
                    }
                    
                    let config = self.config.clone();
                    tokio::spawn(async move {
                        if let Err(e) = config.save().await {
                            tracing::warn!("Failed to save config: {}", e);
                        }
                    });
                }
            }

            AppEvent::PlaylistExpanded { urls } => {
                self.loading_playlists = self.loading_playlists.saturating_sub(1);
                for (url, title) in urls {
                    let mut job = Job::new(&url);
                    job.title = title;
                    let job_id = job.id;
                    self.jobs.push(job);
                    if self.worker_tx.try_send(WorkerCommand::FetchFormats { job_id, url }).is_err() {
                        tracing::warn!("Worker channel full: FetchFormats dropped");
                    }
                }
            }
        }
    }

    pub fn selected_job(&self) -> Option<&Job> {
        self.jobs.get(self.selected_index)
    }

    pub fn status_counts(&self) -> StatusCounts {
        self.jobs.iter().fold(StatusCounts::default(), |mut c, j| {
            match &j.status {
                JobStatus::FetchingFormats => c.fetching += 1,
                JobStatus::Ready { .. } => c.ready += 1,
                JobStatus::Queued => c.queued += 1,
                JobStatus::Downloading { .. } => c.active += 1,
                JobStatus::Completed => c.completed += 1,
                JobStatus::Failed(_) => c.failed += 1,
                JobStatus::Cancelled => {},
            }
            c
        })
    }

    pub fn aggregate_progress(&self) -> Option<(f32, String, String)> {
        let downloading: Vec<_> = self.jobs.iter()
            .filter(|j| matches!(j.status, JobStatus::Downloading { .. }))
            .collect();
        
        if downloading.is_empty() {
            return None;
        }
        
        // Calculate weighted progress based on phase
        let total_progress: f32 = downloading.iter()
            .map(|j| {
                if let JobStatus::Downloading { percent, phase, .. } = &j.status {
                    match phase {
                        DownloadPhase::Video => percent * 0.5,           // 0-50%
                        DownloadPhase::Audio => 50.0 + percent * 0.4,   // 50-90%
                        DownloadPhase::Merging => 90.0 + percent * 0.1, // 90-100%
                        DownloadPhase::Single => *percent,              // 0-100%
                    }
                } else {
                    0.0
                }
            })
            .sum();
        
        let avg_progress = total_progress / downloading.len() as f32;
        
        // Use most recent job's speed and ETA
        let (speed, eta) = downloading.last()
            .and_then(|j| match &j.status {
                JobStatus::Downloading { speed, eta, .. } => Some((speed.clone(), eta.clone())),
                _ => None,
            })
            .unwrap_or_default();
        
        Some((avg_progress, speed, eta))
    }
}

fn is_playlist_url(url: &str) -> bool {
    url.contains("youtube.com/playlist") 
        || url.contains("youtu.be/playlist")
        || (url.contains("youtube.com/watch") && url.contains("&list="))
        || (url.contains("youtu.be/") && url.contains("?list="))
}
