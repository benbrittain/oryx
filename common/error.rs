use thiserror::Error;

#[derive(Debug, Error)]
pub enum OryxError {
    #[error("Digest format not valid: {0}")]
    InvalidDigest(String),
}
