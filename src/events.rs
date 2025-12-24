use std::path::PathBuf;
use serde::Deserialize;
use uuid::Uuid;

pub type JobId = Uuid;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Format {
    pub format_id: String,
    #[serde(default)]
    pub resolution: Option<String>,
    #[serde(default)]
    pub ext: String,
    #[serde(default)]
    pub vcodec: Option<String>,
    #[serde(default)]
    pub acodec: Option<String>,
    #[serde(default)]
    pub filesize: Option<u64>,
    #[serde(default)]
    pub filesize_approx: Option<u64>,
    #[serde(default)]
    pub tbr: Option<f64>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

impl Format {
    pub fn display_resolution(&self) -> String {
        if let (Some(w), Some(h)) = (self.width, self.height) {
            format!("{}x{}", w, h)
        } else if let Some(res) = &self.resolution {
            res.clone()
        } else {
            "audio".to_string()
        }
    }

    pub fn display_size(&self) -> String {
        let size = self.filesize.or(self.filesize_approx);
        match size {
            Some(b) if b >= 1024 * 1024 * 1024 => format!("{:.2} GiB", b as f64 / (1024.0 * 1024.0 * 1024.0)),
            Some(b) if b >= 1024 * 1024 => format!("{:.2} MiB", b as f64 / (1024.0 * 1024.0)),
            Some(b) if b >= 1024 => format!("{:.2} KiB", b as f64 / 1024.0),
            Some(b) => format!("{} B", b),
            None => "~".to_string(),
        }
    }

    pub fn display_bitrate(&self) -> String {
        match self.tbr {
            Some(br) => format!("{:.0} kbps", br),
            None => "~".to_string(),
        }
    }

    pub fn is_video(&self) -> bool {
        self.vcodec.as_ref().map(|v| v != "none").unwrap_or(false)
    }

    pub fn is_audio_only(&self) -> bool {
        !self.is_video() && self.acodec.as_ref().map(|a| a != "none").unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct FormatPopupState {
    pub job_index: usize,
    pub formats: Vec<Format>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub audio_only: bool,
    pub apply_to_all: bool,
}

impl FormatPopupState {
    pub fn new(job_index: usize, formats: Vec<Format>) -> Self {
        Self {
            job_index,
            formats,
            selected: 0,
            scroll_offset: 0,
            audio_only: false,
            apply_to_all: false,
        }
    }

    pub fn filtered_formats(&self) -> Vec<&Format> {
        self.formats
            .iter()
            .filter(|f| {
                if self.audio_only {
                    f.is_audio_only()
                } else {
                    f.is_video()
                }
            })
            .collect()
    }

    pub fn selected_format(&self) -> Option<&Format> {
        self.filtered_formats().get(self.selected).copied()
    }
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    AddUrl(String),
    StartDownloads,
    OpenFormatPopup,
    CloseFormatPopup,
    FormatSelectNext,
    FormatSelectPrev,
    ToggleAudioOnly,
    ToggleApplyToAll,
    ConfirmFormat,
    CancelJob(JobId),
    RemoveJob(JobId),
    SelectNext,
    SelectPrev,
    ToggleInputMode,
    ToggleHelp,
    Quit,

    JobStarted { id: JobId },
    FormatsReady { id: JobId, title: String, formats: Vec<Format> },
    JobProgress { id: JobId, percent: f32, speed: String, eta: String },
    JobCompleted { id: JobId, path: PathBuf },
    JobFailed { id: JobId, error: String },
}

#[derive(Debug, Clone)]
pub enum WorkerCommand {
    FetchFormats { job_id: JobId, url: String },
    StartJob { job: Box<Job>, format_id: String },
    CancelJob(JobId),
    Shutdown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    FetchingFormats,
    Ready { formats: Vec<Format> },
    Queued,
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
    pub formats: Vec<Format>,
    pub selected_format: Option<Format>,
    pub output_path: Option<PathBuf>,
}

impl Job {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            url: url.into(),
            title: None,
            duration: None,
            status: JobStatus::FetchingFormats,
            formats: Vec::new(),
            selected_format: None,
            output_path: None,
        }
    }

    pub fn display_name(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.url)
    }

    pub fn can_select_format(&self) -> bool {
        matches!(self.status, JobStatus::Ready { .. } | JobStatus::Queued) && !self.formats.is_empty()
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            JobStatus::FetchingFormats | JobStatus::Downloading { .. }
        )
    }
}
