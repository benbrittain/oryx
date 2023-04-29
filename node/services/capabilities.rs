use tokio::sync::mpsc;
use tonic::{Request, Response, Status};
use log::info;

#[derive(Debug, Default)]
pub struct CapabilitiesService {}

#[tonic::async_trait]
impl protos::Capabilities for CapabilitiesService {
    async fn get_capabilities(
        &self,
        request: Request<protos::GetCapabilitiesRequest>,
    ) -> Result<Response<protos::ServerCapabilities>, Status> {
        info!("Instance: {}", request.get_ref().instance_name);
        let api_version = protos::SemVer {
            major: 2,
            minor: 0,
            patch: 0,
            prerelease: String::default(),
        };

        let cache_capabilities = protos::CacheCapabilities {
            digest_functions: vec![protos::digest_function::Value::Sha256.into()],
            action_cache_update_capabilities: Some(protos::ActionCacheUpdateCapabilities {
                update_enabled: true,
            }),
            cache_priority_capabilities: None,
            max_batch_total_size_bytes: 0,
            symlink_absolute_path_strategy: 0,
            supported_compressors: vec![],
            supported_batch_update_compressors: vec![],
        };

        let exec_caps = protos::ExecutionCapabilities {
            digest_function: protos::digest_function::Value::Sha256.into(),
            exec_enabled: true,
            execution_priority_capabilities: None,
            supported_node_properties: vec![],
        };

        let caps = protos::ServerCapabilities {
            cache_capabilities: Some(cache_capabilities),
            execution_capabilities: Some(exec_caps),
            deprecated_api_version: None,
            low_api_version: Some(api_version.clone()),
            high_api_version: Some(api_version.clone()),
        };
        Ok(Response::new(caps))
    }
}
