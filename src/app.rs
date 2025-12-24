use tokio::sync::mpsc;

use crate::config::Config;
use crate::events::{AppEvent, FormatPopupState, Job, JobStatus, WorkerCommand};

pub struct App {
    pub jobs: Vec<Job>,
    pub selected_index: usize,
    pub input_buffer: String,
    pub input_mode: bool,
    pub show_help: bool,
    pub should_quit: bool,
    pub format_popup: Option<FormatPopupState>,
    pub config: Config,
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
            should_quit: false,
            format_popup: None,
            config,
            worker_tx,
        }
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::AddUrl(url) => {
                if !url.trim().is_empty() {
                    let job = Job::new(url.trim());
                    let job_id = job.id;
                    let url = job.url.clone();
                    self.jobs.push(job);
                    let _ = self.worker_tx.try_send(WorkerCommand::FetchFormats { job_id, url });
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
                            let _ = self.worker_tx.try_send(WorkerCommand::StartJob {
                                job: Box::new(job.clone()),
                                format_id: fmt.format_id.clone(),
                            });
                        }
                    }
                }
            }

            AppEvent::CancelJob(id) => {
                let _ = self.worker_tx.try_send(WorkerCommand::CancelJob(id));
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

            AppEvent::Quit => {
                let _ = self.worker_tx.try_send(WorkerCommand::Shutdown);
                self.should_quit = true;
            }

            AppEvent::JobStarted { id } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.status = JobStatus::Downloading {
                        percent: 0.0,
                        speed: "--".into(),
                        eta: "--".into(),
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

            AppEvent::JobProgress { id, percent, speed, eta } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.status = JobStatus::Downloading { percent, speed, eta };
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
        }
    }

    pub fn selected_job(&self) -> Option<&Job> {
        self.jobs.get(self.selected_index)
    }

    pub fn fetching_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| matches!(j.status, JobStatus::FetchingFormats))
            .count()
    }

    pub fn ready_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| matches!(j.status, JobStatus::Ready { .. }))
            .count()
    }

    pub fn queued_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.status == JobStatus::Queued)
            .count()
    }

    pub fn active_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| matches!(j.status, JobStatus::Downloading { .. }))
            .count()
    }

    pub fn completed_count(&self) -> usize {
        self.jobs
            .iter()
            .filter(|j| j.status == JobStatus::Completed)
            .count()
    }
}
