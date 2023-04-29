use async_trait::async_trait;

pub mod insecure;

#[async_trait]
pub trait ExecutionEngine {}
