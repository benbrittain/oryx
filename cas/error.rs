use thiserror::Error;

#[derive(Debug, Error)]
pub enum CasError {
    #[error("Unknown")]
    Unknown,
    #[error("Blob not found for: {0:?}")]
    BlobNotFound(common::Digest),
}
