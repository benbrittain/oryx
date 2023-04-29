use common::Digest;
use async_trait::async_trait;

mod memory;
mod error;

pub use memory::InMemory;
pub use error::CasError;

#[async_trait]
pub trait ContentAddressableStorage {
    async fn write_blob(&self, digest: Digest, data: &[u8]) -> Result<(), CasError>;

    async fn read_blob(&self, digest: Digest) -> Result<Vec<u8>, CasError>;

    async fn has_blob(&self, digest: Digest) -> Result<bool, CasError>;
}
