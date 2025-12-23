use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use directories::ProjectDirs;
use color_eyre::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub output_dir: PathBuf,
    pub output_template: String,
    pub max_concurrent_downloads: usize,
    pub default_format: String,
}

impl Default for Config {
    fn default() -> Self {
        let video_dir = directories::UserDirs::new()
            .and_then(|d| d.video_dir().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        Self {
            output_dir: video_dir,
            output_template: "%(title)s.%(ext)s".into(),
            max_concurrent_downloads: 3,
            default_format: "bestvideo+bestaudio/best".into(),
        }
    }
}

impl Config {
    pub fn config_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "oxidlp", "oxidlp")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let Some(path) = Self::config_path() else {
            return Ok(Self::default());
        };

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let Some(path) = Self::config_path() else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

pub fn check_ytdlp() -> Result<String> {
    let output = std::process::Command::new("yt-dlp")
        .arg("--version")
        .output()?;

    if !output.status.success() {
        color_eyre::eyre::bail!("yt-dlp is not installed or not in PATH");
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(version)
}
