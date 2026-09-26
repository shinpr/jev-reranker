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

    #[error("invalid model score for unit {index}: expected a finite number in [0, 1]")]
    InvalidRerankScore { index: usize },

    #[error("score count {actual} does not match scoring unit count {expected}")]
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

    #[error("could not install the skill to {path}: {source}")]
    SkillInstall {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("{variable} is not set; pass --path to choose where to install the skill")]
    MissingHome { variable: &'static str },

    #[error("TYPESAFE_API_KEY is required for non-empty input")]
    MissingCredential,

    #[cfg(not(debug_assertions))]
    #[error("the test endpoint is unavailable in release builds")]
    ReleaseTestEndpoint,

    #[cfg(debug_assertions)]
    #[error("the test endpoint must be a literal loopback HTTP URL")]
    InvalidTestEndpoint,

    #[error("could not initialize the blocking HTTP client")]
    HttpClient,

    #[error("batch {batch} request timed out")]
    HttpTimeout { batch: usize },

    #[error("batch {batch} request failed due to a transport error")]
    HttpTransport { batch: usize },

    #[error("batch {batch} returned HTTP status {status}{}", describe_message(message.as_deref()))]
    HttpStatus {
        batch: usize,
        status: u16,
        message: Option<String>,
    },

    #[error("batch {batch} returned retryable HTTP status {status} after retry exhaustion")]
    HttpRetryExhausted { batch: usize, status: u16 },

    #[error("batch {batch} returned an invalid response or answer set")]
    InvalidResponse { batch: usize },

    #[error(
        "batch {batch}: input item {index} exceeds the Jev request token limit on its own; shorten its text, context fields, or the query"
    )]
    ItemTooLarge { batch: usize, index: usize },
}

fn describe_message(message: Option<&str>) -> String {
    message.map_or_else(String::new, |message| format!(": {message}"))
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
            | Self::ScoreCountMismatch { .. }
            | Self::Serialization { .. }
            | Self::Io { .. }
            | Self::SkillInstall { .. }
            | Self::MissingHome { .. }
            | Self::MissingCredential
            | Self::HttpClient
            | Self::HttpTimeout { .. }
            | Self::HttpTransport { .. }
            | Self::HttpStatus { .. }
            | Self::HttpRetryExhausted { .. }
            | Self::InvalidResponse { .. }
            | Self::ItemTooLarge { .. } => 1,
            #[cfg(debug_assertions)]
            Self::InvalidTestEndpoint => 1,
            #[cfg(not(debug_assertions))]
            Self::ReleaseTestEndpoint => 1,
        }
    }
}
