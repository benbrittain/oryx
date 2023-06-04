use crate::error::CasError;
use async_trait::async_trait;
use common::Digest;
use sha2::{Digest as _, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::{fs::File, io::AsyncReadExt, sync::Mutex};
use crate::ContentAddressableStorage;

#[derive(Default, Debug, Clone)]
pub struct InMemory {
    cas: Arc<Mutex<HashMap<Digest, Vec<u8>>>>,
}


#[async_trait]
impl crate::ContentAddressableStorage for InMemory {
    async fn write_blob(&self, data: &[u8], expected_digest: Option<Digest>) -> Result<Digest, CasError> {
        log::info!("write: {}: {:?}", data.len(), expected_digest);

        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash_buf = hasher.finalize();
        let hex_hash = base16ct::lower::encode_string(&hash_buf);
        let actual_digest = Digest::from_str(&format!("{}:{}", hex_hash, data.len())).expect("oh no");
        if let Some(expected_digest) = expected_digest {
            if actual_digest != expected_digest {
                return Err(CasError::InvalidDigest(actual_digest, expected_digest));
            }
        }

        let mut cas = self.cas.lock().await;
        cas.insert(actual_digest.clone(), data.to_vec());
        Ok(actual_digest)
    }

    async fn read_blob(&self, digest: Digest) -> Result<Vec<u8>, CasError> {
        let cas = self.cas.lock().await;
        let data = cas.get(&digest).ok_or_else(|| CasError::BlobNotFound(digest.clone()))?;
        log::info!("read: {}: {:?}", digest, data);
        Ok(data.to_vec())
    }

    async fn has_blob(&self, digest: &Digest) -> Result<bool, CasError> {
        let cas = self.cas.lock().await;
        let r = cas.contains_key(digest);
        log::info!("check: {} / {}", digest, r);
        Ok(r)
    }
}
