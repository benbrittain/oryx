use thiserror::Error;
use common::Digest;

#[derive(Debug, Error)]
pub enum CasError {
    #[error("Unknown")]
    Unknown,
    #[error("Blob not found for: {0:?}")]
    BlobNotFound(common::Digest),
    #[error("Blob not found for: {0:?}")]
    IoError(#[from] std::io::Error),
    #[error("Digest {0} does not match expected {1}")]
    InvalidDigest(Digest, Digest),
}
