use crate::oryx_test;
use common::Digest;
use prost::Message;
use protos::{
    longrunning::operation::Result::Response,
    re::batch_update_blobs_request::Request as BlobRequest,
    re::batch_update_blobs_response::Response as BlobResponse,
    rpc::{Code, Status},
};
use std::future::{ready, Future, IntoFuture, Ready};
use std::str::FromStr;
use tokio_stream::StreamExt;
use tonic::Request;

#[tokio::test]
async fn invalid_no_action_digest() {
    oryx_test(|channel| async move {
        let mut exec_client = protos::ExecutionClient::new(channel);
        let mut response = exec_client
            .execute(Request::new(protos::re::ExecuteRequest {
                instance_name: "".to_string(),
                action_digest: None,
                execution_policy: None,
                results_cache_policy: None,
                skip_cache_lookup: false,
            }))
            .await
            .unwrap()
            .into_inner();

        let mut got_response = false;
        while let Some(op) = response.next().await {
            got_response = true;
            let op = op.unwrap();
            assert!(op.name.starts_with("operations/"));
            let Response(result) = op.result.unwrap() else { todo!() };
            let resp: protos::re::ExecuteResponse =
                Message::decode(result.value.as_slice()).unwrap();
            let status = resp.status.unwrap();

            // Should fail with invalid argument since no action digest was passed.
            assert_eq!(status.code, Code::InvalidArgument.into());
        }

        assert!(got_response);
    })
    .await;
}

#[tokio::test]
async fn blob_precondition_failure() {
    oryx_test(|channel| async move {
        let mut exec_client = protos::ExecutionClient::new(channel.clone());
        let mut cas_client = protos::ContentAddressableStorageClient::new(channel);

        let missing_digest: protos::re::Digest = Digest::from_str("aaaa:5").unwrap().into();
        let mut response = exec_client
            .execute(Request::new(protos::re::ExecuteRequest {
                instance_name: "".to_string(),
                action_digest: Some(missing_digest.into()),
                execution_policy: None,
                results_cache_policy: None,
                skip_cache_lookup: false,
            }))
            .await
            .unwrap()
            .into_inner();

        let mut got_response = false;
        while let Some(op) = response.next().await {
            got_response = true;
            let op = op.unwrap();
            assert!(op.name.starts_with("operations/"));
            let Response(result) = op.result.unwrap() else { todo!() };
            let resp: protos::re::ExecuteResponse =
                Message::decode(result.value.as_slice()).unwrap();
            let status = resp.status.unwrap();

            // Should fail with Failed Precondition since the fake action digest was never uploaded
            assert_eq!(status.code, Code::FailedPrecondition.into());
        }
        assert!(got_response);
    })
    .await;
}

mod gemsbok {
    use crate::execute::{BlobRequest, Request};
    use anyhow::Error;
    use common::Digest;
    use prost::Message;
    use protos::longrunning::operation::Result::Response;
    use protos::{ContentAddressableStorageClient, ExecutionClient};
    use sha2::{Digest as _, Sha256};
    use std::str::FromStr;
    use tokio_stream::StreamExt;
    use tonic::transport::Channel;

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
        pub async fn execute(
            &mut self,
            action_digest: ActionDigest,
        ) -> Result<ActionResult, Error> {
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

                let stderr = dbg!(std::str::from_utf8(&resp.stderr_raw)?);
                let stdout = dbg!(std::str::from_utf8(&resp.stdout_raw)?);
                return Ok(ActionResult {
                    exit_code: resp.exit_code,
                    stderr: stderr.into(),
                    stdout: stdout.into(),
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
            let encoded_digest =
                Digest::from_str(&format!("{}:{}", hex_hash, encoded.len())).unwrap();

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
    }

    pub struct File {
        name: String,
        contents: Vec<u8>,
    }

    pub struct Directory {
        files: Vec<File>,
        dirs: Vec<Directory>,
    }

    impl Directory {
        pub fn root() -> Self {
            Directory {
                files: vec![],
                dirs: vec![],
            }
        }

        pub fn file(mut self, name: &str, contents: &[u8]) -> Self {
            self.files.push(File {
                name: name.to_owned(),
                contents: contents.to_owned(),
            });
            self
        }
    }
}
