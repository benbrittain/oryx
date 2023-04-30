use crate::*;
use anyhow::{anyhow, Error};
use openat2::*;
use std::os::fd::RawFd;
use tempdir::TempDir;
use tokio::process;

#[derive(Debug)]
pub struct Insecure {
    root_fd: RawFd,
}

impl Insecure {
    pub fn new() -> Result<Self, Error> {
        if !has_openat2_cached() {
            return Err(anyhow!(
                "Insecure is not particularly secure, but it does use openat2. \
                    If your system does not support that, please file an issue."
            ));
        }
        let tmp_dir = TempDir::new("oryx-insecure")?;
        let root_path = tmp_dir.path();
        let mut how = OpenHow::new(libc::O_CLOEXEC | libc::O_DIRECTORY, 0);
        how.resolve |= ResolveFlags::NO_SYMLINKS;
        let root_fd = openat2(None, &root_path, &how)?;
        Ok(Insecure { root_fd })
    }
}

#[async_trait]
impl ExecutionEngine for Insecure {
    async fn run_command(
        &self,
        command: Command,
        dir: DirectoryLayout,
    ) -> Result<CommandResponse, Error> {
        let binary = &command.arguments[0];
        let args = &command.arguments[1..];
        let mut command = process::Command::new(binary);
        command.args(args);
        todo!()
    }
}
