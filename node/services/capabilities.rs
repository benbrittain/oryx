use tonic::{Request, Response, Status};
use tracing::info;

#[derive(Debug, Default)]
pub struct CapabilitiesService {}

#[tonic::async_trait]
impl protos::Capabilities for CapabilitiesService {
    async fn get_capabilities(
        &self,
        request: Request<protos::re::GetCapabilitiesRequest>,
    ) -> Result<Response<protos::re::ServerCapabilities>, Status> {
        info!("Instance: '{}'", request.get_ref().instance_name);
        let api_version = protos::semver::SemVer {
            major: 2,
            minor: 0,
            patch: 0,
            prerelease: String::default(),
        };

        let cache_capabilities = protos::re::CacheCapabilities {
            digest_functions: vec![protos::re::digest_function::Value::Sha256.into()],
            action_cache_update_capabilities: Some(protos::re::ActionCacheUpdateCapabilities {
                update_enabled: true,
            }),
            cache_priority_capabilities: None,
            max_batch_total_size_bytes: 0,
            symlink_absolute_path_strategy: 0,
            supported_compressors: vec![],
            supported_batch_update_compressors: vec![],
        };

        let exec_caps = protos::re::ExecutionCapabilities {
            digest_function: protos::re::digest_function::Value::Sha256.into(),
            exec_enabled: true,
            execution_priority_capabilities: None,
            supported_node_properties: vec![],
        };

        let caps = protos::re::ServerCapabilities {
            cache_capabilities: Some(cache_capabilities),
            execution_capabilities: Some(exec_caps),
            deprecated_api_version: None,
            low_api_version: Some(api_version.clone()),
            high_api_version: Some(api_version.clone()),
        };
        Ok(Response::new(caps))
    }
}
