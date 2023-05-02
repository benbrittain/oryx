use crate::*;
use anyhow::{anyhow, Error};
use cas::ContentAddressableStorage;
use openat2::*;
use std::os::fd::RawFd;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempdir::TempDir;
use tokio::{fs::File, io::AsyncWriteExt, process};
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct Insecure<C> {
    cas: C,
}

impl<C: ContentAddressableStorage> Insecure<C> {
    pub fn new(cas: C) -> Result<Self, Error> {
        Ok(Insecure { cas })
    }
}

fn get_root_relative(root_path: &PathBuf, path: &Path) -> PathBuf {
    let mut new_path: PathBuf = root_path.clone();
    new_path.push(path);
    new_path
}

#[async_trait]
impl<C: ContentAddressableStorage> ExecutionBackend for Insecure<C> {
    #[instrument(skip(self))]
    async fn run_command(
        &self,
        command: Command,
        dir: DirectoryLayout,
    ) -> Result<CommandResponse, Error> {
        log::info!("{command:#?}");
        log::info!("{dir:#?}");

        // Create a temporary directory and write all files from the cas there
        let tmp_dir = TempDir::new("oryx-insecure")?;
        let root_path = tmp_dir.path().to_path_buf();
        log::info!("Insecure directory: {root_path:#?}");
        for entry in dir.files {
            let path = get_root_relative(&root_path, &entry.path);
            if let Some(prefix) = path.parent() {
                std::fs::create_dir_all(prefix)?;
            }
            let mut file = File::create(&path).await?;
            log::info!("entry: {path:#?}");
            log::info!("digest: {}", entry.digest);
            let data = self.cas.read_blob(entry.digest).await?;
            log::info!("data length: {}", data.len());
            log::info!("file: {file:?}");
            file.write_all(&data).await?;
            file.flush().await?;
        }

        let binary = &command.arguments[0];
        let args = &command.arguments[1..];
        let output = process::Command::new(binary)
            .current_dir(root_path.clone())
            .args(args)
            .env_clear()
            .envs(command.env_vars)
            .output()
            .await?;

        // Verify outputs were created and get their hash
        let mut output_paths = vec![];
        for path in dir.output_paths {
            let local_path = get_root_relative(&root_path, &path);
            if local_path.exists() {
                output_paths.push((path, local_path));
            }
        }

        // TODO Remove this!
        std::mem::forget(tmp_dir);

        log::info!("Command Output: {output:?}");
        Ok(CommandResponse {
            exit_status: output.status.code().unwrap(),
            output_paths,
            stderr: output.stderr,
            stdout: output.stdout,
        })
    }
}
