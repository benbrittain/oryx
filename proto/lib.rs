pub use build::bazel::remote::execution::v2::{
    self as re,
    action_cache_client::ActionCacheClient,
    action_cache_server::{ActionCache, ActionCacheServer},
    capabilities_client::CapabilitiesClient,
    capabilities_server::{Capabilities, CapabilitiesServer},
    content_addressable_storage_client::ContentAddressableStorageClient,
    content_addressable_storage_server::{
        ContentAddressableStorage, ContentAddressableStorageServer,
    },
    execution_client::ExecutionClient,
    execution_server::{Execution, ExecutionServer},
};
pub use build::bazel::semver;
pub use google::bytestream::{
    self as bytestream,
    byte_stream_client::ByteStreamClient,
    byte_stream_server::{ByteStream, ByteStreamServer},
};
pub use google::longrunning::{
    self as longrunning,
    operations_client::OperationsClient,
    operations_server::{Operations, OperationsServer},
};
pub use google::rpc;

mod google {
    pub mod longrunning {
        tonic::include_proto!("google.longrunning");
    }
    pub mod bytestream {
        tonic::include_proto!("google.bytestream");
    }
    pub mod rpc {
        tonic::include_proto!("google.rpc");
    }
}

mod build {
    pub mod bazel {
        pub mod semver {
            tonic::include_proto!("build.bazel.semver");
        }
        pub mod remote {
            pub mod execution {
                pub mod v2 {
                    tonic::include_proto!("build.bazel.remote.execution.v2");
                }
            }
        }
    }
}
