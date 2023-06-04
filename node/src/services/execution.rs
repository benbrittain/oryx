use anyhow::{anyhow, Error};
use cas::ContentAddressableStorage;
use execution_engine::{
    Entry, ExecuteError, ExecuteStage, ExecuteStatus, ExecutionBackend, ExecutionEngine,
};
use futures::future::BoxFuture;
use futures::StreamExt;
use prost::Message;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{info, instrument, trace};

pub static EXEC_OP_METADATA: &'static str =
    "type.googleapis.com/build.bazel.remote.execution.v2.ExecuteOperationMetadata";
pub static EXEC_RESP: &'static str =
    "type.googleapis.com/build.bazel.remote.execution.v2.ExecuteResponse";
pub static PRECONDITION_FAILURE: &'static str =
    "type.googleapis.com/com.google.rpc.PreconditionFailure";

pub struct ExecutionService<C, B> {
    instance: String,
    cas: C,
    engine: ExecutionEngine<B, C>,
}

impl<C: ContentAddressableStorage, B> ExecutionService<C, B> {
    pub fn new(instance: &str, cas: C, engine: ExecutionEngine<B, C>) -> Self {
        ExecutionService {
            instance: instance.to_string(),
            cas,
            engine,
        }
    }
}

pub async fn get_proto<P: prost::Message + Default, C: ContentAddressableStorage>(
    cas: C,
    digest: common::Digest,
) -> Result<P, ExecuteError> {
    let blob = cas
        .read_blob(digest.clone().into())
        .await
        .map_err(|_| ExecuteError::BlobNotFound(digest.clone()))?;
    let proto = P::decode(&mut std::io::Cursor::new(blob))
        .map_err(|_| ExecuteError::InvalidArgument(format!("{digest} was not a valid proto")))?;
    Ok(proto)
}

fn create_mapping<'a, C: ContentAddressableStorage>(
    mapping: &'a mut execution_engine::DirectoryLayout,
    dir: protos::re::Directory,
    cas: C,
    root: PathBuf,
) -> BoxFuture<'a, Result<(), ExecuteError>> {
    Box::pin(async move {
        assert_eq!(dir.symlinks.len(), 0);
        for file in dir.files {
            let mut path = root.clone();
            path.push(&file.name);
            mapping.entries.push(execution_engine::Entry::File {
                digest: file
                    .digest
                    .ok_or(ExecuteError::InvalidArgument(String::from(
                        "no digest in entry",
                    )))?
                    .into(),
                path,
                executable: file.is_executable,
            });
        }
        for directory_node in &dir.directories {
            let node_digest =
                directory_node
                    .digest
                    .clone()
                    .ok_or(ExecuteError::Internal(format!(
                        "No digest in node for directory: {}",
                        root.display()
                    )))?;
            let dir: protos::re::Directory = get_proto(cas.clone(), node_digest.into()).await?;
            let mut new_root = root.clone();
            new_root.push(&directory_node.name);
            create_mapping(mapping, dir, cas.clone(), new_root).await?;
        }
        Ok(())
    })
}

#[tonic::async_trait]
impl<C: ContentAddressableStorage, B: ExecutionBackend> protos::Execution
    for ExecutionService<C, B>
{
    type ExecuteStream = ReceiverStream<Result<protos::longrunning::Operation, Status>>;

    #[instrument(skip_all)]
    async fn execute(
        &self,
        request: Request<protos::re::ExecuteRequest>,
    ) -> Result<Response<Self::ExecuteStream>, Status> {
        let request = request.into_inner();
        if request.instance_name != self.instance {
            return Err(Status::permission_denied(
                "Request sent to invalid instance: {self.instance}.",
            ));
        }

        let cas = self.cas.clone();
        let cas2 = self.cas.clone();
        let mut exec_events = self
            .engine
            .execute(move || {
                let request = request.clone();
                let cas = cas.clone();
                async move {
                    let action_digest = request.action_digest.ok_or(
                        ExecuteError::InvalidArgument(String::from("no action digest specified")),
                    )?;
                    let action: protos::re::Action =
                        get_proto(cas.clone(), action_digest.clone().into()).await?;
                    trace!("{action:#?}");

                    let command_digest =
                        action.command_digest.ok_or(ExecuteError::InvalidArgument(
                            String::from("Invalid Action: no command digest specified."),
                        ))?;
                    let command: protos::re::Command =
                        get_proto(cas.clone(), command_digest.into()).await?;
                    trace!("{command:#?}");

                    let root_digest =
                        action
                            .input_root_digest
                            .ok_or(ExecuteError::InvalidArgument(format!(
                                "Invalid Action: no root digest specified."
                            )))?;
                    let root_directory: protos::re::Directory =
                        get_proto(cas.clone(), root_digest.into()).await?;
                    trace!("{root_directory:#?}");

                    // Collect a command for the execution engine
                    let cmd = execution_engine::Command {
                        arguments: command.arguments,
                        env_vars: command
                            .environment_variables
                            .iter()
                            .map(|ev| (ev.name.clone(), ev.value.clone()))
                            .collect(),
                    };

                    // Collect the filesystem information for the execution engine
                    let mut dir_layout = execution_engine::DirectoryLayout::default();
                    create_mapping(
                        &mut dir_layout,
                        root_directory,
                        cas.clone(),
                        PathBuf::default(),
                    )
                    .await
                    .map_err(|e| {
                        ExecuteError::Internal(format!(
                            "Failed to execute mapping collection: {e:?}"
                        ))
                    })?;
                    dbg!(&dir_layout);

                    if !command.output_directories.is_empty() || !command.output_files.is_empty() {
                        return Err(ExecuteError::InvalidArgument(format!(
                            "Only RBE v2.1+ Supported. Use output_paths"
                        )));
                    }

                    // Collect output path information
                    if command.output_paths.is_empty() {
                        return Err(ExecuteError::InvalidArgument(format!(
                            "No output_paths were specified."
                        )));
                    }
                    for path in command.output_paths {
                        dir_layout.output_paths.push(path.into());
                    }

                    Ok((action_digest.into(), cmd, dir_layout))
                }
            })
            .map_err(|e| Status::unknown(format!("Failed to execute: {e}")))?;

        // TODO is there an easy way to map over this instead of making another channel?
        let (tx, rx) = mpsc::channel(32);
        while let Some(event) = exec_events.recv().await {
            info!("{:?}", event);
            let op = convert_to_op(event);
            tx.send(op)
                .await
                .map_err(|e| Status::unknown(format!("Failed to execute command: {e}")))?;
        }
        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(output_stream as Self::ExecuteStream))
    }

    type WaitExecutionStream = ReceiverStream<Result<protos::longrunning::Operation, Status>>;

    #[instrument(skip_all)]
    async fn wait_execution(
        &self,
        request: Request<protos::re::WaitExecutionRequest>,
    ) -> Result<Response<Self::WaitExecutionStream>, Status> {
        Err(Status::not_found("WaitExecution: Not yet implemented"))
    }
}

fn convert_to_op(
    exec_status: ExecuteStatus,
) -> Result<protos::longrunning::Operation, tonic::Status> {
    // Assemble the metadata
    let metadata = protos::re::ExecuteOperationMetadata {
        stage: match exec_status.stage {
            ExecuteStage::Queued => protos::re::execution_stage::Value::Queued,
            ExecuteStage::Running => protos::re::execution_stage::Value::Executing,
            ExecuteStage::Done(_) => protos::re::execution_stage::Value::Completed,
            ExecuteStage::Error(_) => protos::re::execution_stage::Value::Unknown,
        }
        .into(),
        action_digest: exec_status.action_digest.map(|ad| ad.into()),
        // TODO stream metadata
        stdout_stream_name: String::from(""),
        stderr_stream_name: String::from(""),
    };

    let (done, result) = match exec_status.stage {
        ExecuteStage::Queued => (false, None),
        ExecuteStage::Running => (false, None),
        ExecuteStage::Error(status) => {
            let status = match status {
                ExecuteError::InvalidArgument(info) => protos::rpc::Status {
                    code: protos::rpc::Code::InvalidArgument.into(),
                    message: format!("Invalid Argument: {info}"),
                    ..Default::default()
                },
                ExecuteError::BlobNotFound(digest) => {
                    // In the case of a missing input or command, the server SHOULD additionally
                    // send a [PreconditionFailure][google.rpc.PreconditionFailure] error detail
                    // where, for each requested blob not present in the CAS, there is a
                    // `Violation` with a `type` of `MISSING` and a `subject` of
                    // `"blobs/{hash}/{size}"` indicating the digest of the missing blob.
                    let hash = digest.hash();
                    let size = digest.size_bytes();
                    let precondition = protos::rpc::PreconditionFailure {
                        violations: vec![protos::rpc::precondition_failure::Violation {
                            r#type: String::from("MISSING"),
                            subject: format!("blobs/{hash}/{size}"),
                            description: String::from("blob not found"),
                        }],
                    };
                    protos::rpc::Status {
                        code: protos::rpc::Code::FailedPrecondition.into(),
                        message: format!("{digest} not found"),
                        details: vec![prost_types::Any {
                            type_url: PRECONDITION_FAILURE.to_string(),
                            value: precondition.encode_to_vec(),
                        }],
                    }
                }
                ExecuteError::Internal(info) => protos::rpc::Status {
                    code: protos::rpc::Code::Internal.into(),
                    message: format!("Internal Failure: {info}."),
                    ..Default::default()
                },
            };

            let response = protos::re::ExecuteResponse {
                // If the status has a code other than `OK`, it indicates that the action did
                // not finish execution. For example, if the operation times out during
                // execution, the status will have a `DEADLINE_EXCEEDED` code. Servers MUST
                // use this field for errors in execution, rather than the error field on the
                // `Operation` object.
                result: None,
                // If the status code is other than `OK`, then the result MUST NOT be cached.
                // For an error status, the `result` field is optional; the server may
                // populate the output-, stdout-, and stderr-related fields if it has any
                // information available, such as the stdout and stderr of a timed-out action.
                cached_result: false,
                status: Some(status),
                server_logs: HashMap::new(),
                message: String::from(""),
            };
            (true, Some(response))
        }
        ExecuteStage::Done(resp) => {
            let execution_metadata = protos::re::ExecutedActionMetadata {
                ..Default::default()
            };

            // Collect outputs from the finished execution
            let mut output_files = vec![];
            let mut output_directories = vec![];
            for entry in resp.output_paths {
                match entry {
                    Entry::File {
                        path,
                        digest,
                        executable,
                    } => {
                        output_files.push(protos::re::OutputFile {
                            path: path.display().to_string(),
                            digest: Some(digest.into()),
                            is_executable: executable,
                            // The contents of the file if inlining was requested. The server
                            // SHOULD NOT inline file contents unless requested by the client in
                            // the [GetActionResultRequest][build.bazel.remote.execution.v2.GetActionResultRequest]
                            // message. The server MAY omit inlining, even if requested, and MUST do so if inlining
                            // would cause the response to exceed message size limits.
                            contents: vec![],
                            node_properties: None,
                        });
                    }
                    Entry::Directory { path, digest } => {
                        assert!(path.is_relative());
                        output_directories.push(protos::re::OutputDirectory {
                            path: path.display().to_string(),
                            tree_digest: Some(digest.into()),
                            is_topologically_sorted: false,
                        });
                    }
                }
            }

            info!("output: {:#?}", output_files);

            let response = protos::re::ExecuteResponse {
                result: Some(protos::re::ActionResult {
                    output_files,
                    output_file_symlinks: vec![],
                    output_symlinks: vec![],
                    output_directories,
                    output_directory_symlinks: vec![],
                    exit_code: resp.exit_status,
                    execution_metadata: Some(execution_metadata),
                    stdout_digest: None,
                    stderr_digest: None,
                    stdout_raw: resp.stdout,
                    stderr_raw: resp.stderr,
                }),
                cached_result: false,
                status: Some(protos::rpc::Status {
                    code: 0,
                    message: "".to_string(),
                    details: vec![],
                }),
                server_logs: HashMap::new(),
                message: String::from(""),
            };
            (true, Some(response))
        }
    };
    // Name pattern defined by longrunning proto & in remote_execution.proto under WaitExecution
    let name = format!("operations/{}", exec_status.uuid);
    Ok(protos::longrunning::Operation {
        name,
        done,
        metadata: Some(prost_types::Any {
            type_url: EXEC_OP_METADATA.to_string(),
            value: metadata.encode_to_vec(),
        }),
        // Errors discovered during creation of the `Operation` will be reported
        // as gRPC Status errors, while errors that occurred while running the
        // action will be reported in the `status` field of the `ExecuteResponse`. The
        // server MUST NOT set the `error` field of the `Operation` proto.
        result: result.map(|result| {
            // There should never be a scenario where we are attempting to return a response
            // to an unfinished execution.
            assert!(done);
            protos::longrunning::operation::Result::Response(prost_types::Any {
                type_url: EXEC_RESP.to_string(),
                value: result.encode_to_vec(),
            })
        }),
    })
}
