//! Shared error type for llmsc-core.

/// Errors from core operations (driver/client boundaries).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("VM driver: {0}")]
    Vm(String),
    #[error("Incus: {0}")]
    Incus(String),
    #[error("not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, Error>;
