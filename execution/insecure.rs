use crate::*;
use anyhow::{anyhow, Error};
use cas::ContentAddressableStorage;
use futures::future::{BoxFuture, FutureExt};
use openat2::*;
use prost::Message;
use std::os::fd::RawFd;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempdir::TempDir;
use tokio::io::AsyncReadExt;
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

    async fn add_symlink(
        &self,
        root_path: &Path,
        path: &Path,
        target: &Path,
    ) -> Result<Entry, Error> {
        Ok(Entry::Symlink {
            path: path.strip_prefix(&root_path).unwrap().to_path_buf(),
            target: target.strip_prefix(&root_path).unwrap().to_path_buf(),
        })
    }

    async fn add_file(&self, root_path: &Path, path: &Path) -> Result<Entry, Error> {
        let mut file = tokio::fs::File::open(&path).await?;
        let mut buf = vec![];
        file.read_to_end(&mut buf).await?;
        let digest = self.cas.write_blob(&buf, None).await?;
        Ok(Entry::File {
            path: path.strip_prefix(&root_path).unwrap().to_path_buf(),
            digest,
            executable: false,
        })
    }

    fn add_dir<'a>(
        &'a self,
        root_path: &'a Path,
        path: &'a Path,
        children: &'a mut Vec<protos::re::Directory>,
    ) -> BoxFuture<'a, Result<protos::re::Directory, Error>> {
        Box::pin(async move {
            let mut files = vec![];
            let mut directories = vec![];

            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                if entry.path().is_symlink() {
                    todo!();
                } else if entry.path().is_file() {
                    let Entry::File{path, digest, executable} = self.add_file(root_path, &entry.path()).await? else { unreachable!() };
                    files.push(protos::re::FileNode {
                        name: entry.file_name().to_str().unwrap().to_string(),
                        digest: Some(digest.into()),
                        is_executable: executable,
                        node_properties: None,
                    })
                } else if entry.path().is_dir() {
                    let dir = self.add_dir(root_path, &entry.path(), children).await?;
                    children.push(dir.clone());
                    let proto_buf = dir.encode_to_vec();
                    let digest = self.cas.write_blob(&proto_buf, None).await?;
                    directories.push(protos::re::DirectoryNode {
                        name: entry.file_name().to_str().unwrap().to_string(),
                        digest: Some(digest.into()),
                    })
                } else {
                    unreachable!();
                }
            }

            Ok(protos::re::Directory {
                files,
                directories,
                symlinks: vec![],
                node_properties: None,
            })
        })
    }
}

fn get_root_relative(root_path: &Path, path: &Path) -> PathBuf {
    let mut new_path: PathBuf = root_path.to_path_buf();
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
    ) -> Result<ExecuteResponse, Error> {
        log::info!("{command:#?}");

        // Create a temporary directory and write all files from the cas there
        let tmp_dir = TempDir::new("oryx-insecure")?;
        let root_path = tmp_dir.path().to_path_buf();
        log::info!("Insecure directory: {root_path:#?}");
        for entry in dir.entries {
            match entry {
                Entry::Symlink { path, target } => {
                    let path = get_root_relative(&root_path, &path);
                    let target = get_root_relative(&root_path, &target);
                    std::os::unix::fs::symlink(path, target)?;
                }
                Entry::Directory { path, .. } => {
                    let path = get_root_relative(&root_path, &path);
                    std::fs::create_dir_all(path)?;
                }
                Entry::File {
                    digest,
                    executable,
                    path,
                } => {
                    let path = get_root_relative(&root_path, &path);
                    if let Some(prefix) = path.parent() {
                        std::fs::create_dir_all(prefix)?;
                    }
                    let mut file = File::create(&path).await?;
                    if executable {
                        let metadata = file.metadata().await?;
                        let mut permissions = metadata.permissions();
                        permissions.set_mode(0o777);
                        tokio::fs::set_permissions(&path, permissions).await?;
                    }
                    eprintln!("entry: {path:#?}");
                    log::info!("digest: {}", digest);
                    let data = self.cas.read_blob(digest).await?;
                    eprintln!("data length: {}", data.len());
                    eprintln!("file: {file:?}");
                    file.write_all(&data).await?;
                    file.flush().await?;
                }
            }
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
        log::info!("{:?}", &output);

        // Verify outputs were created and get their hash
        let mut entries = vec![];
        for path in dir.output_paths {
            let global_path = get_root_relative(&root_path, &path);
            let mut children = vec![];
            if global_path.is_symlink() {
                let symlink_path = global_path.read_link().expect("read_link call failed");
                let symlink_path = get_root_relative(&root_path, &symlink_path);
                entries.push(
                    self.add_symlink(&root_path, &global_path, &symlink_path)
                        .await?,
                );
            } else if global_path.is_dir() {
                let root = self
                    .add_dir(&root_path, &global_path, &mut children)
                    .await?;
                let tree = protos::re::Tree {
                    root: Some(root),
                    children,
                };
                let proto_buf = tree.encode_to_vec();
                let digest = self.cas.write_blob(&proto_buf, None).await?;
                entries.push(Entry::Directory { path, digest });
            } else if global_path.is_file() {
                entries.push(self.add_file(&root_path, &global_path).await?);
            } else {
                panic!("path of unknown type");
            }
        }

        // TODO Remove this!
        // Only here so I can look in the insecure folders in /tmp during testing.
        std::mem::forget(tmp_dir);

        log::info!("Command Output: {output:?}");
        Ok(ExecuteResponse {
            exit_status: output.status.code().unwrap(),
            output_paths: entries,
            stderr: output.stderr,
            stdout: output.stdout,
        })
    }
}
