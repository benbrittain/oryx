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
use crate::gemsbok::*;

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

#[tokio::test]
async fn basic_req() {
    oryx_test(|channel| async move {
        let mut client = Gemsbok::new(channel);
        let command_digest = client
            .add_command(&["/bin/sh", "-c", "echo magic > out.txt"], &["out.txt"])
            .await
            .unwrap();
        let root_dir_digest = client.add_directory(Directory::root()).await.unwrap();
        let action_digest = client
            .add_action(command_digest, root_dir_digest)
            .await
            .unwrap();
        let result = client.execute(action_digest).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(
            result.directory,
            Directory::root().file("out.txt", b"magic\n")
        );
    })
    .await;
}

#[tokio::test]
async fn basic_req_with_file() {
    oryx_test(|channel| async move {
        let mut client = Gemsbok::new(channel);
        let command_digest = client
            .add_command(&["/bin/sh", "-c", "ls > out.txt"], &["out.txt"])
            .await
            .unwrap();
        let root_dir_digest = client.add_directory(Directory::root().file("file0.txt", b"0")).await.unwrap();
        let action_digest = client
            .add_action(command_digest, root_dir_digest)
            .await
            .unwrap();
        let result = client.execute(action_digest).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(
            result.directory,
            Directory::root().file("out.txt", b"file0.txt\nout.txt\n")
        );
    })
    .await;
}
