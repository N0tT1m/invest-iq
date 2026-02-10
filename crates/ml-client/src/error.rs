use thiserror::Error;

#[derive(Error, Debug)]
pub enum MLError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Model not loaded")]
    ModelNotLoaded,

    #[error("Timeout")]
    Timeout,

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Other error: {0}")]
    Other(String),
}

pub type MLResult<T> = Result<T, MLError>;
