use crate::error::CasError;
use async_trait::async_trait;
use common::Digest;
use sha2::{Digest as _, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::{fs::File, io::AsyncReadExt, sync::Mutex};

#[derive(Default, Debug, Clone)]
pub struct InMemory {
    cas: Arc<Mutex<HashMap<Digest, Vec<u8>>>>,
}

#[async_trait]
impl crate::ContentAddressableStorage for InMemory {
    async fn write_blob(&self, digest: Digest, data: &[u8]) -> Result<(), CasError> {
        let mut cas = self.cas.lock().await;
        log::info!("write: {}: {:?}", digest, data.len());
        cas.insert(digest, data.to_vec());
        Ok(())
    }

    async fn read_blob(&self, digest: Digest) -> Result<Vec<u8>, CasError> {
        let cas = self.cas.lock().await;
        let data = cas.get(&digest).expect("nth");
        //            .ok_or_else(|| CasError::BlobNotFound(digest.clone()))?;
        log::info!("read: {}: {:?}", digest, data);
        Ok(data.to_vec())
    }

    async fn has_blob(&self, digest: &Digest) -> Result<bool, CasError> {
        let cas = self.cas.lock().await;
        //       log::info!("check: {:?}", cas.keys());
        let r = cas.contains_key(digest);
        log::info!("check: {} / {}", digest, r);
        Ok(r)
    }

    async fn add_from_file(&self, path: &Path) -> Result<Digest, CasError> {
        let mut file = File::open(path).await?;
        let mut buf = vec![];
        file.read_to_end(&mut buf).await?;
        log::info!("Adding {:?} to CAS", path);

        // TODO generic hashing infra
        let mut hasher = Sha256::new();
        hasher.update(&buf);
        let hash_buf = hasher.finalize();
        let hex_hash = base16ct::lower::encode_string(&hash_buf);
        let digest = Digest::from_str(&format!("{}:{}", hex_hash, buf.len())).expect("oh no");
        log::info!("buf: {:?}", buf);
        log::info!("digest: {}", digest);

        {
            let mut cas = self.cas.lock().await;
            cas.insert(digest.clone(), buf.to_vec());
        }

        let blob = self.read_blob(digest.clone()).await;
        log::info!("readback: {:?}", blob);

        Ok(digest)
    }
}
