use async_trait::async_trait;
use common::Digest;
use std::path::Path;

mod error;
mod memory;

pub use error::CasError;
pub use memory::InMemory;

#[async_trait]
pub trait ContentAddressableStorage: Clone + Send + Sync + 'static {
    async fn write_blob(&self, digest: Digest, data: &[u8]) -> Result<(), CasError>;

    async fn read_blob(&self, digest: Digest) -> Result<Vec<u8>, CasError>;

    async fn has_blob(&self, digest: &Digest) -> Result<bool, CasError>;

    async fn add_from_file(&self, path: &Path) -> Result<Digest, CasError>;
}
