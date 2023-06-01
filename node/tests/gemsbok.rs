use anyhow::Error;
use common::Digest;
use prost::Message;
use protos::{
    longrunning::operation::Result::Response,
    re::batch_update_blobs_request::Request as BlobRequest, ContentAddressableStorageClient,
    ExecutionClient,
};
use sha2::{Digest as _, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
};
use tokio_stream::StreamExt;
use tonic::{transport::Channel, Request};

// Some light typesafety for the various digests.
#[derive(Debug)]
pub struct ActionDigest(pub Digest);
#[derive(Debug)]
pub struct CommandDigest(pub Digest);
#[derive(Debug)]
pub struct DirectoryDigest(pub Digest);

/// A simple client interface for interacting with the RBE protocol
pub struct Gemsbok {
    exec: ExecutionClient<Channel>,
    cas: ContentAddressableStorageClient<Channel>,
}

impl Gemsbok {
    pub fn new(channel: Channel) -> Self {
        Gemsbok {
            exec: ExecutionClient::new(channel.clone()),
            cas: ContentAddressableStorageClient::new(channel.clone()),
        }
    }

    /// Create a Directory message and upload to CAS returning the digest.
    pub async fn add_directory(&mut self, root: Directory) -> Result<DirectoryDigest, Error> {
        let mut files = vec![];
        for file in root.files {
            let file_digest = self.upload_blob(&file.contents).await?;
            let node = protos::re::FileNode {
                name: file.name,
                digest: Some(file_digest.into()),
                ..Default::default()
            };
            files.push(node);
        }

        let root = protos::re::Directory {
            files,
            ..Default::default()
        };
        Ok(DirectoryDigest(self.upload_proto(root).await?))
    }

    /// Create a Command message and upload to CAS returning the digest.
    pub async fn add_command(
        &mut self,
        args: &[&str],
        out_paths: &[&str],
    ) -> Result<CommandDigest, Error> {
        let cmd = protos::re::Command {
            arguments: args.iter().map(|a| String::from(*a)).collect(),
            output_paths: out_paths.iter().map(|a| String::from(*a)).collect(),
            ..Default::default()
        };

        Ok(CommandDigest(self.upload_proto(cmd).await?))
    }

    /// Create a Action message and upload to CAS returning the digest.
    pub async fn add_action(
        &mut self,
        command_digest: CommandDigest,
        input_root_digest: DirectoryDigest,
    ) -> Result<ActionDigest, Error> {
        let action = protos::re::Action {
            command_digest: Some(command_digest.0.into()),
            input_root_digest: Some(input_root_digest.0.into()),
            ..Default::default()
        };

        Ok(ActionDigest(self.upload_proto(action).await?))
    }

    /// Execute a action.
    pub async fn execute(&mut self, action_digest: ActionDigest) -> Result<ActionResult, Error> {
        let mut response = self
            .exec
            .execute(Request::new(protos::re::ExecuteRequest {
                instance_name: "".to_string(),
                action_digest: Some(action_digest.0.into()),
                execution_policy: None,
                results_cache_policy: None,
                skip_cache_lookup: false,
            }))
            .await?
            .into_inner();
        while let Some(op) = response.next().await {
            let op = op.unwrap();
            // If `done` == `false`, neither `error` nor `response` is set.
            // If `done` == `true`, exactly one of `error` or `response` is set.
            if !op.done {
                continue;
            }

            assert!(op.name.starts_with("operations/"));
            let Response(result) = op.result.unwrap() else { todo!() };
            let resp: protos::re::ExecuteResponse =
                Message::decode(result.value.as_slice()).unwrap();
            let status = resp.status.unwrap();

            // Should succeed
            assert_eq!(status.code, protos::rpc::Code::Ok.into());
            let resp = resp.result.unwrap();

            // TODO handle digests
            assert!(resp.stderr_digest.is_none());
            assert!(resp.stdout_digest.is_none());

            let stderr = std::str::from_utf8(&resp.stderr_raw)?;
            let stdout = std::str::from_utf8(&resp.stdout_raw)?;

            let mut directory = Directory::root();
            for file in resp.output_files {
                let path = PathBuf::from(file.path);
                let contents = self.get_blob(file.digest.unwrap().into()).await?;
                directory.add_file(&path, &contents);
            }

            // TODO other output response fields

            return Ok(ActionResult {
                exit_code: resp.exit_code,
                stderr: stderr.into(),
                stdout: stdout.into(),
                directory,
            });
        }
        Err(anyhow::anyhow!("invalid"))
    }

    async fn upload_blob(&mut self, encoded: &[u8]) -> Result<Digest, Error> {
        let encoded = encoded.to_vec();
        let mut hasher = Sha256::new();
        hasher.update(&encoded);
        let hash_buf = hasher.finalize();
        let hex_hash = base16ct::lower::encode_string(&hash_buf);
        let encoded_digest = Digest::from_str(&format!("{}:{}", hex_hash, encoded.len())).unwrap();

        let mut response_digests: Vec<(Digest, i32)> = self
            .cas
            .batch_update_blobs(Request::new(protos::re::BatchUpdateBlobsRequest {
                requests: vec![BlobRequest {
                    digest: Some(encoded_digest.into()),
                    data: encoded,
                    compressor: Default::default(),
                }],
                instance_name: "".to_string(),
            }))
            .await
            .unwrap()
            .into_inner()
            .responses
            .into_iter()
            .map(|r| (r.digest.unwrap().into(), r.status.unwrap().code))
            .collect();

        assert_eq!(response_digests[0].1, protos::rpc::Code::Ok.into());
        Ok(response_digests[0].0.clone())
    }

    async fn get_blob(&mut self, digest: Digest) -> Result<Vec<u8>, Error> {
        let mut responses: Vec<(Digest, Vec<u8>)> = self
            .cas
            .batch_read_blobs(Request::new(protos::re::BatchReadBlobsRequest {
                instance_name: "".to_string(),
                acceptable_compressors: vec![],
                digests: vec![digest.clone().into()],
            }))
            .await
            .unwrap()
            .into_inner()
            .responses
            .into_iter()
            .map(|r| (r.digest.unwrap().into(), r.data))
            .collect();
        assert_eq!(responses[0].0, digest);

        Ok(responses[0].1.clone())
    }

    async fn upload_proto<P: prost::Message>(&mut self, message: P) -> Result<Digest, Error> {
        let encoded = P::encode_to_vec(&message);
        self.upload_blob(&encoded).await
    }
}

#[derive(Debug)]
pub struct ActionResult {
    pub exit_code: i32,
    pub stderr: Vec<u8>,
    pub stdout: Vec<u8>,
    pub directory: Directory,
}

#[derive(Debug, PartialEq)]
pub struct File {
    name: String,
    contents: Vec<u8>,
}

#[derive(Debug, PartialEq)]
pub struct Directory {
    files: Vec<File>,
    dirs: HashMap<String, Directory>,
}

impl Directory {
    pub fn root() -> Self {
        Directory {
            files: vec![],
            dirs: HashMap::new(),
        }
    }

    pub fn add_file(&mut self, path: &Path, contents: &[u8]) {
        assert!(path.is_relative());
        let components = path.components().collect::<Vec<_>>();
        if components.len() == 1 {
            self.files.push(File {
                name: path.file_name().unwrap().to_str().unwrap().to_string(),
                contents: contents.to_vec(),
            });
        }

        // TODO finish
        assert_eq!(components.len(), 1);
    }

    pub fn file(mut self, name: &str, contents: &[u8]) -> Self {
        self.files.push(File {
            name: name.to_owned(),
            contents: contents.to_owned(),
        });
        self
    }
}
