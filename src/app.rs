use tokio::sync::mpsc;
use crate::events::{AppEvent, Job, JobId, JobStatus, WorkerCommand};
use crate::config::Config;

pub struct App {
    pub jobs: Vec<Job>,
    pub selected_index: usize,
    pub input_buffer: String,
    pub input_mode: bool,
    pub show_help: bool,
    pub should_quit: bool,
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
            config,
            worker_tx,
        }
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::AddUrl(url) => {
                if !url.trim().is_empty() {
                    let job = Job::new(url.trim());
                    self.jobs.push(job);
                }
            }

            AppEvent::StartDownload => {
                for job in &self.jobs {
                    if job.status == JobStatus::Pending {
                        let _ = self.worker_tx.try_send(WorkerCommand::StartJob(job.clone()));
                    }
                }
            }

            AppEvent::CancelJob(id) => {
                let _ = self.worker_tx.try_send(WorkerCommand::CancelJob(id));
            }

            AppEvent::RemoveJob(id) => {
                self.jobs.retain(|j| j.id != id);
                if self.selected_index >= self.jobs.len() && !self.jobs.is_empty() {
                    self.selected_index = self.jobs.len() - 1;
                }
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

            AppEvent::ToggleHelp => {
                self.show_help = !self.show_help;
            }

            AppEvent::Quit => {
                let _ = self.worker_tx.try_send(WorkerCommand::Shutdown);
                self.should_quit = true;
            }

            AppEvent::JobQueued { id, url } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.url = url;
                }
            }

            AppEvent::JobStarted { id } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.status = JobStatus::FetchingMetadata;
                }
            }

            AppEvent::JobMetadata { id, title, duration } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.title = Some(title);
                    job.duration = duration;
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

            AppEvent::Tick | AppEvent::Resize(_, _) => {}
        }
    }

    pub fn selected_job(&self) -> Option<&Job> {
        self.jobs.get(self.selected_index)
    }

    pub fn pending_count(&self) -> usize {
        self.jobs.iter().filter(|j| j.status == JobStatus::Pending).count()
    }

    pub fn active_count(&self) -> usize {
        self.jobs.iter().filter(|j| j.is_active()).count()
    }

    pub fn completed_count(&self) -> usize {
        self.jobs.iter().filter(|j| j.status == JobStatus::Completed).count()
    }
}
