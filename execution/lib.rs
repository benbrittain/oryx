use anyhow::Error;
use async_trait::async_trait;
use std::path::PathBuf;
use common::Digest;

pub mod insecure;

#[derive(Default, Debug)]
pub struct Command {
    pub arguments: Vec<String>,
    pub env_vars: Vec<(String, String)>,
}

#[derive(Default, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub digest: Digest,
    pub executable: bool,
}

#[derive(Default, Debug)]
pub struct DirectoryLayout {
    pub files: Vec<Entry>,
}

pub struct CommandResponse {
    exit_status: i32,
    stderr: Vec<u8>,
    stdout: Vec<u8>,
}

#[async_trait]
pub trait ExecutionEngine: Send + Sync + 'static {
    async fn run_command(
        &self,
        command: Command,
        dir: DirectoryLayout,
    ) -> Result<CommandResponse, Error>;
}
