use async_trait::async_trait;
use cas::ContentAddressableStorage;
use common::Digest;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{event, span, Instrument, Level};
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
        let span = span!(Level::TRACE, "execute backend");
        let (tx, rx) = mpsc::channel(32);
        let backend = self.backend.clone();
        let uuid = Uuid::new_v4();

        //
        // TODO we really need to capture any errors in here
        //
        tokio::spawn(
            async move {
                let setup_span = span!(Level::TRACE, "setup function");
                // Run the actual command using the backend.
                match setup_func().instrument(setup_span).await {
                    Ok((action_digest, cmd, layout)) => {
                        tx.send(ExecuteStatus {
                            uuid: uuid,
                            action_digest: Some(action_digest.clone()),
                            stage: ExecuteStage::Queued,
                        })
                        .await?;

                        // TODO scheduling logic here
                        tx.send(ExecuteStatus {
                            uuid: uuid,
                            action_digest: Some(action_digest.clone()),
                            stage: ExecuteStage::Running,
                        })
                        .await?;

                        // Run the actual command using the backend.
                        let run_span = span!(Level::TRACE, "run command");
                        match backend.run_command(cmd, layout).instrument(run_span).await {
                            Ok(resp) => {
                                event!(Level::TRACE, exit_status = resp.exit_status, "result");
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
            }
            .instrument(span),
        );

        Ok(rx)
    }
}
