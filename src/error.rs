use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid option --{option}: {message}")]
    Usage { option: String, message: String },

    #[error("invalid input: {message}")]
    InvalidInput { message: String },

    #[error("invalid JSON input: {source}")]
    Json {
        #[source]
        source: serde_json::Error,
    },

    #[error("invalid input item {index} field '{field}': {message}")]
    InvalidItem {
        index: usize,
        field: String,
        message: String,
    },

    #[error("invalid rerank score for item {index}: expected a finite number in [0, 1]")]
    InvalidRerankScore { index: usize },

    #[error("computed score for item {index} is not finite")]
    NonFiniteComputedScore { index: usize },

    #[error("rerank score count {actual} does not match document count {expected}")]
    ScoreCountMismatch { expected: usize, actual: usize },

    #[error("serialization failed: {source}")]
    Serialization {
        #[source]
        source: serde_json::Error,
    },

    #[error("I/O failed: {source}")]
    Io {
        #[source]
        source: std::io::Error,
    },

    #[error("runtime error: {message}")]
    Runtime { message: String },
}

impl AppError {
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::Usage { .. } => 2,
            Self::InvalidInput { .. }
            | Self::Json { .. }
            | Self::InvalidItem { .. }
            | Self::InvalidRerankScore { .. }
            | Self::NonFiniteComputedScore { .. }
            | Self::ScoreCountMismatch { .. }
            | Self::Serialization { .. }
            | Self::Io { .. }
            | Self::Runtime { .. } => 1,
        }
    }
}
