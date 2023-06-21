use async_trait::async_trait;
use cas::ContentAddressableStorage;
use common::Digest;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{Command, DirectoryLayout, ExecuteError, ExecuteResponse, ExecutionBackend};

#[derive(Debug)]
pub struct ExecuteStatus {
    pub uuid: Uuid,
    pub action_digest: Option<Digest>,
    pub stage: ExecuteStage,
}

#[derive(Debug)]
pub enum ExecuteStage {
    Queued,
    Running,
    Done(ExecuteResponse),
    Error(ExecuteError),
}

pub struct ExecutionEngine<B> {
    backend: B,
}

impl<B: ExecutionBackend> ExecutionEngine<B> {
    pub fn new(backend: B) -> Self {
        ExecutionEngine { backend }
    }

    pub fn execute<Exec>(
        &self,
        setup_func: impl Fn() -> Exec + Send + Sync + 'static,
    ) -> Result<mpsc::Receiver<ExecuteStatus>, ExecuteError>
    where
        Exec: Future<Output = Result<(Digest, Command, DirectoryLayout), ExecuteError>> + Send,
    {
        let (tx, rx) = mpsc::channel(32);
        let backend = self.backend.clone();
        let uuid = Uuid::new_v4();

        //
        // TODO we really need to capture any errors in here
        //
        tokio::spawn(async move {
            log::info!("================= {uuid} | Start =================");
            // Run the actual command using the backend.
            match setup_func().await {
                Ok((action_digest, cmd, layout)) => {
                    log::info!("================= {uuid} | Queued =================");
                    tx.send(ExecuteStatus {
                        uuid: uuid,
                        action_digest: Some(action_digest.clone()),
                        stage: ExecuteStage::Queued,
                    })
                    .await?;

                    // TODO scheduling logic here
                    log::info!("================= {uuid} | Running =================");
                    tx.send(ExecuteStatus {
                        uuid: uuid,
                        action_digest: Some(action_digest.clone()),
                        stage: ExecuteStage::Running,
                    })
                    .await?;

                    // Run the actual command using the backend.
                    match backend.run_command(cmd, layout).await {
                        Ok(resp) => {
                            log::info!("================= {uuid} | Done =================");
                            tx.send(ExecuteStatus {
                                uuid: uuid,
                                action_digest: Some(action_digest.clone()),
                                stage: ExecuteStage::Done(ExecuteResponse {
                                    exit_status: resp.exit_status,
                                    output_paths: resp.output_paths,
                                    stderr: resp.stderr,
                                    stdout: resp.stdout,
                                }),
                            })
                            .await?;
                        }
                        Err(err) => {
                            tx.send(ExecuteStatus {
                                uuid: uuid,
                                action_digest: None,
                                stage: ExecuteStage::Error(err),
                            })
                            .await?;
                        }
                    }
                }
                Err(err) => {
                    tx.send(ExecuteStatus {
                        uuid: uuid,
                        action_digest: None,
                        stage: ExecuteStage::Error(err),
                    })
                    .await?;
                }
            }
            // TODO caching logic here
            anyhow::Ok(())
        });

        Ok(rx)
    }
}
