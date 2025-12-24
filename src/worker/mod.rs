use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::config::Config;
use crate::events::{AppEvent, JobId, WorkerCommand};

mod ytdlp;

type ActiveJobsMap = HashMap<JobId, CancellationToken>;

pub struct WorkerPool {
    config: Arc<Config>,
    command_rx: mpsc::Receiver<WorkerCommand>,
    event_tx: mpsc::Sender<AppEvent>,
    active_jobs: Arc<Mutex<ActiveJobsMap>>,
}

impl WorkerPool {
    pub fn new(
        config: Arc<Config>,
        command_rx: mpsc::Receiver<WorkerCommand>,
        event_tx: mpsc::Sender<AppEvent>,
    ) -> Self {
        Self {
            config,
            command_rx,
            event_tx,
            active_jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(mut self) {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(
            self.config.max_concurrent_downloads,
        ));

        while let Some(cmd) = self.command_rx.recv().await {
            match cmd {
                WorkerCommand::FetchFormats { job_id, url } => {
                    let event_tx = self.event_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = ytdlp::fetch_formats(job_id, &url, event_tx.clone()).await {
                            let _ = event_tx
                                .send(AppEvent::JobFailed {
                                    id: job_id,
                                    error: e.to_string(),
                                })
                                .await;
                        }
                    });
                }

                WorkerCommand::StartJob { job, format_id } => {
                    let permit = semaphore.clone().acquire_owned().await;
                    if permit.is_err() {
                        continue;
                    }
                    let permit = permit.unwrap();

                    let cancel_token = CancellationToken::new();
                    {
                        let mut jobs = self.active_jobs.lock().await;
                        jobs.insert(job.id, cancel_token.clone());
                    }

                    let event_tx = self.event_tx.clone();
                    let config = self.config.clone();
                    let active_jobs = self.active_jobs.clone();
                    let job_id = job.id;

                    tokio::spawn(async move {
                        let _permit = permit;

                        let _ = event_tx.send(AppEvent::JobStarted { id: job_id }).await;

                        let result = ytdlp::download(
                            &job,
                            &format_id,
                            &config,
                            event_tx.clone(),
                            cancel_token,
                        )
                        .await;

                        match result {
                            Ok(path) => {
                                let _ = event_tx
                                    .send(AppEvent::JobCompleted { id: job_id, path })
                                    .await;
                            }
                            Err(e) => {
                                let _ = event_tx
                                    .send(AppEvent::JobFailed {
                                        id: job_id,
                                        error: e.to_string(),
                                    })
                                    .await;
                            }
                        }

                        let mut jobs = active_jobs.lock().await;
                        jobs.remove(&job_id);
                    });
                }

                WorkerCommand::CancelJob(id) => {
                    let jobs = self.active_jobs.lock().await;
                    if let Some(token) = jobs.get(&id) {
                        token.cancel();
                    }
                }

                WorkerCommand::Shutdown => {
                    let jobs = self.active_jobs.lock().await;
                    for token in jobs.values() {
                        token.cancel();
                    }
                    break;
                }
            }
        }
    }
}
