use crate::oryx_test;
use common::Digest;
use futures::Future;
use prost::Message;
use protos::{
    longrunning::operation::Result::Response,
    rpc::{Code, Status},
};
use std::str::FromStr;
use tokio_stream::StreamExt;
use tonic::Request;

#[tokio::test]
async fn invalid_no_action_digest() {
    let missing_digest: protos::re::Digest = Digest::from_str("aaaa:5").unwrap().into();
    let mut got_response = false;

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
        while let Some(op) = response.next().await {
            got_response = true;
            let op = op.unwrap();
            assert!(op.name.starts_with("operations/"));
            let Response(result) = op.result.unwrap() else { todo!() };
            let resp: protos::re::ExecuteResponse =
                Message::decode(result.value.as_slice()).unwrap();
            let status = resp.status.unwrap();

            // Should fail with invalid argemnt since no action digest was passed.
            assert_eq!(status.code, Code::InvalidArgument.into());
        }

        assert!(got_response);
    })
    .await;
}
