use std::path::PathBuf;
use uuid::Uuid;

pub type JobId = Uuid;

#[derive(Debug, Clone)]
pub enum AppEvent {
    AddUrl(String),
    StartDownload,
    CancelJob(JobId),
    RemoveJob(JobId),
    SelectNext,
    SelectPrev,
    ToggleHelp,
    Quit,

    JobQueued { id: JobId, url: String },
    JobStarted { id: JobId },
    JobMetadata { id: JobId, title: String, duration: Option<u64> },
    JobProgress { id: JobId, percent: f32, speed: String, eta: String },
    JobCompleted { id: JobId, path: PathBuf },
    JobFailed { id: JobId, error: String },

    Tick,
    Resize(u16, u16),
}

#[derive(Debug, Clone)]
pub enum WorkerCommand {
    StartJob(Job),
    CancelJob(JobId),
    Shutdown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    Pending,
    FetchingMetadata,
    Downloading { percent: f32, speed: String, eta: String },
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: JobId,
    pub url: String,
    pub title: Option<String>,
    pub duration: Option<u64>,
    pub status: JobStatus,
    pub output_path: Option<PathBuf>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Job {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            url: url.into(),
            title: None,
            duration: None,
            status: JobStatus::Pending,
            output_path: None,
            created_at: chrono::Utc::now(),
        }
    }

    pub fn display_name(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.url)
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status, JobStatus::Pending | JobStatus::FetchingMetadata | JobStatus::Downloading { .. })
    }
}
