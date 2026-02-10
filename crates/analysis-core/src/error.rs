use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalysisError {
    #[error("Insufficient data: {0}")]
    InsufficientData(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Calculation error: {0}")]
    CalculationError(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}
