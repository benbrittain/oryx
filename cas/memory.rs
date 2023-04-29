use crate::error::CasError;
use async_trait::async_trait;
use common::Digest;
use std::collections::HashMap;
use tokio::sync::Mutex;

#[derive(Default, Debug)]
pub struct InMemory {
    cas: Mutex<HashMap<Digest, Vec<u8>>>,
}

#[async_trait]
impl crate::ContentAddressableStorage for InMemory {
    async fn write_blob(&self, digest: Digest, data: &[u8]) -> Result<(), CasError> {
        let mut cas = self.cas.lock().await;
        log::info!("write: {}", digest);
        cas.insert(digest, data.to_vec());
        Ok(())
    }

    async fn read_blob(&self, digest: Digest) -> Result<Vec<u8>, CasError> {
        let cas = self.cas.lock().await;
        log::info!("read: {}", digest);
        let data = cas.get(&digest).ok_or_else(|| CasError::BlobNotFound(digest))?;
        Ok(data.to_vec())
    }

    async fn has_blob(&self, digest: Digest) -> Result<bool, CasError> {
        let cas = self.cas.lock().await;
        let r = cas.contains_key(&digest);
        log::info!("check: {} / {}", digest, r);
        Ok(r)
    }
}
