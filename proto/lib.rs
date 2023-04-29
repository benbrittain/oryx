pub use build::bazel::remote::execution::v2::action_cache_server::*;
pub use build::bazel::remote::execution::v2::capabilities_server::*;
pub use build::bazel::remote::execution::v2::content_addressable_storage_server::*;
pub use build::bazel::remote::execution::v2::execution_server::*;
pub use build::bazel::remote::execution::v2::*;
pub use build::bazel::semver::SemVer;
pub use google::bytestream::byte_stream_server::*;
pub use google::bytestream::*;
pub use google::longrunning;
pub use google::longrunning::operations_server::*;
pub use google::longrunning::*;
pub use google::rpc::Status;

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

pub mod build {
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
