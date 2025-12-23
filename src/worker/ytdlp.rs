use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use color_eyre::Result;

use crate::config::Config;
use crate::events::{AppEvent, Job};

pub async fn download(
    job: &Job,
    config: &Config,
    event_tx: mpsc::Sender<AppEvent>,
    cancel: CancellationToken,
) -> Result<PathBuf> {
    let output_template = config.output_dir.join(&config.output_template);
    
    let mut child = Command::new("yt-dlp")
        .arg("--newline")
        .arg("--progress")
        .arg("--no-colors")
        .arg("-f")
        .arg(&config.default_format)
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
                        } else if let Some(title) = parse_title(&line_content) {
                            let _ = event_tx.send(AppEvent::JobMetadata {
                                id: job.id,
                                title,
                                duration: None,
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
        line[idx + 3..].trim().split_whitespace().next().unwrap_or("--").to_string()
    } else {
        "--".to_string()
    };

    Some(Progress { percent, speed, eta })
}

fn parse_title(line: &str) -> Option<String> {
    if line.contains("[info]") && line.contains("title:") {
        let title = line.split("title:").nth(1)?.trim().to_string();
        return Some(title);
    }
    
    if line.contains("[download] Destination:") {
        let path = line.split("Destination:").nth(1)?.trim();
        let filename = PathBuf::from(path)
            .file_stem()?
            .to_string_lossy()
            .to_string();
        return Some(filename);
    }

    None
}
