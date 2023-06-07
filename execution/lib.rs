use anyhow::Error;
use async_trait::async_trait;
use cas::ContentAddressableStorage;
use common::Digest;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;

pub mod insecure;

#[derive(Default, Debug)]
pub struct Command {
    pub arguments: Vec<String>,
    pub env_vars: Vec<(String, String)>,
}

/// Information on a digest reified into the filesystem.
#[derive(Clone, Debug)]
pub enum Entry {
    File {
        path: PathBuf,
        digest: Digest,
        executable: bool,
    },
    Directory {
        path: PathBuf,
        digest: Digest,
    },
    Symlink {
        path: PathBuf,
        target: PathBuf,
    },
}

/// Description of how to layout execution directory structure
#[derive(Default, Debug)]
pub struct DirectoryLayout {
    /// Entries for laying out directory structure before execution.
    pub entries: Vec<Entry>,
    /// Expected paths to be generated by execution.
    pub output_paths: Vec<PathBuf>,
}

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

#[derive(Debug)]
pub enum ExecuteError {
    /// One or more arguments are invalid.
    InvalidArgument(String),
    /// A Blob was not found when setting up the action was requested. The client may be able to fix the errors and retry.
    BlobNotFound(Digest),
    /// An internal error occurred in the execution engine or the worker.
    Internal(String),
}

#[derive(Debug)]
pub struct ExecuteResponse {
    pub exit_status: i32,
    pub output_paths: Vec<Entry>,
    pub stderr: Vec<u8>,
    pub stdout: Vec<u8>,
}

pub struct ExecutionEngine<B, C> {
    backend: B,
    cas: C,
}

impl<B: ExecutionBackend, C: ContentAddressableStorage> ExecutionEngine<B, C> {
    pub fn new(backend: B, cas: C) -> Self {
        ExecutionEngine { backend, cas }
    }

    pub fn execute<Exec>(
        &self,
        setup_func: impl Fn() -> Exec + Send + Sync + 'static,
    ) -> Result<mpsc::Receiver<ExecuteStatus>, Error>
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
                    let mut resp = backend.run_command(cmd, layout).await?;

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
            // TODO caching logic here
            anyhow::Ok(())
        });

        Ok(rx)
    }
}

#[async_trait]
pub trait ExecutionBackend: Send + Sync + 'static + Clone {
    /// Run a command on the Execution backend.
    ///
    /// Returns an Execute Response
    ///
    ///
    ///
    async fn run_command(
        &self,
        command: Command,
        dir: DirectoryLayout,
    ) -> Result<ExecuteResponse, Error>;
}
