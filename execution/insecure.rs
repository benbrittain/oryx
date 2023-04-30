use crate::*;
use anyhow::{anyhow, Error};
use cas::ContentAddressableStorage;
use openat2::*;
use std::os::fd::RawFd;
use std::path::PathBuf;
use tempdir::TempDir;
use tokio::{fs::File, io::AsyncWriteExt, process};

#[derive(Debug)]
pub struct Insecure<C> {
    cas: C,
}

impl<C: ContentAddressableStorage> Insecure<C> {
    pub fn new(cas: C) -> Result<Self, Error> {
        //if !has_openat2_cached() {
        //    return Err(anyhow!(
        //        "Insecure is not particularly secure, but it does use openat2. \
        //            If your system does not support that, please file an issue."
        //    ));
        //}
        //let mut how = OpenHow::new(libc::O_CLOEXEC | libc::O_DIRECTORY, 0);
        //how.resolve |= ResolveFlags::NO_SYMLINKS;
        //let root_fd = openat2(None, &root_path, &how)?;
        Ok(Insecure { cas })
    }
}

#[async_trait]
impl<C: ContentAddressableStorage> ExecutionEngine for Insecure<C> {
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
        for entry in dir.files {
            let mut path = root_path.clone();
            path.push(entry.path);
            log::info!("entry: {path:#?}");
            if let Some(prefix) = path.parent() {
                std::fs::create_dir_all(prefix)?;
            }
            let mut file = File::create(&path).await?;
            let data = self.cas.read_blob(&entry.digest).await?;
            file.write_all(&data).await?;
        }

        let binary = &command.arguments[0];
        let args = &command.arguments[1..];
        let output = process::Command::new(binary)
            .current_dir(root_path)
            .args(args)
            .env_clear()
            .envs(command.env_vars)
            .output()
            .await?;

        log::info!("info: {output:?}");
        Ok(CommandResponse {
            exit_status: output.status.code().unwrap(),
            stderr: output.stderr,
            stdout: output.stdout,
        })
    }
}
