use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use color_eyre::Result;
use serde::Deserialize;

use crate::config::Config;
use crate::events::{AppEvent, Format, Job, JobId};

#[derive(Debug, Deserialize)]
struct VideoInfo {
    title: String,
    formats: Vec<Format>,
}

pub async fn fetch_formats(
    job_id: JobId,
    url: &str,
    event_tx: mpsc::Sender<AppEvent>,
) -> Result<()> {
    let output = Command::new("yt-dlp")
        .arg("--dump-json")
        .arg("--no-download")
        .arg("--no-warnings")
        .arg(url)
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = event_tx
            .send(AppEvent::JobFailed {
                id: job_id,
                error: stderr.to_string(),
            })
            .await;
        return Ok(());
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let info: VideoInfo = serde_json::from_str(&json_str)?;

    // Filter to useful formats (video with audio, or standalone video/audio)
    let formats: Vec<Format> = info
        .formats
        .into_iter()
        .filter(|f| {
            // Keep formats that have video
            f.is_video() && f.height.is_some()
        })
        .collect();

    let _ = event_tx
        .send(AppEvent::FormatsReady {
            id: job_id,
            title: info.title,
            formats,
        })
        .await;

    Ok(())
}

pub async fn download(
    job: &Job,
    format_id: &str,
    config: &Arc<Config>,
    event_tx: mpsc::Sender<AppEvent>,
    cancel: CancellationToken,
) -> Result<PathBuf> {
    let output_template = config.output_dir.join(&config.output_template);

    let mut child = Command::new("yt-dlp")
        .arg("--newline")
        .arg("--progress")
        .arg("--no-colors")
        .arg("-f")
        .arg(format!("{}+bestaudio/best", format_id))
        .arg("-o")
        .arg(output_template.to_string_lossy().as_ref())
        .arg("--print")
        .arg("after_move:filepath")
        .arg(&job.url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout not captured");
    let mut reader = BufReader::new(stdout).lines();

    let mut final_path: Option<PathBuf> = None;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                child.kill().await?;
                color_eyre::eyre::bail!("Download cancelled");
            }
            result = reader.next_line() => {
                match result {
                    Ok(Some(line_content)) => {
                        if let Some(progress) = parse_progress(&line_content) {
                            let _ = event_tx.send(AppEvent::JobProgress {
                                id: job.id,
                                percent: progress.percent,
                                speed: progress.speed,
                                eta: progress.eta,
                            }).await;
                        } else if !line_content.starts_with('[') && line_content.contains('/') {
                            final_path = Some(PathBuf::from(line_content.trim()));
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        tracing::error!("Error reading stdout: {}", e);
                        break;
                    }
                }
            }
        }
    }

    let status = child.wait().await?;

    if !status.success() {
        color_eyre::eyre::bail!("yt-dlp exited with code: {:?}", status.code());
    }

    final_path.ok_or_else(|| color_eyre::eyre::eyre!("Could not determine output file path"))
}

#[derive(Debug)]
struct Progress {
    percent: f32,
    speed: String,
    eta: String,
}

fn parse_progress(line: &str) -> Option<Progress> {
    if !line.contains("[download]") || !line.contains('%') {
        return None;
    }

    let percent = line
        .split_whitespace()
        .find(|s| s.ends_with('%'))?
        .trim_end_matches('%')
        .parse::<f32>()
        .ok()?;

    let speed = line
        .split_whitespace()
        .find(|s| s.contains("/s"))
        .unwrap_or("--")
        .to_string();

    let eta = if let Some(idx) = line.find("ETA") {
        line[idx + 3..]
            .split_whitespace()
            .next()
            .unwrap_or("--")
            .to_string()
    } else {
        "--".to_string()
    };

    Some(Progress { percent, speed, eta })
}
