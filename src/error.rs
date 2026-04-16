/// Error types with semantic exit codes.
///
/// Every error maps to an exit code (1-4), a machine-readable code, and a
/// recovery suggestion that agents can follow literally.

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("{0}")]
    Transient(String),

    #[error("Rate limited: {0}")]
    RateLimited(String),

    #[error("API error ({code}): {message}")]
    Api { code: String, message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Update failed: {0}")]
    Update(String),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidInput(_) => 3,
            Self::Config(_) => 2,
            Self::RateLimited(_) => 4,
            Self::Transient(_) | Self::Io(_) | Self::Update(_) | Self::Api { .. } => 1,
        }
    }

    pub fn error_code(&self) -> &str {
        match self {
            Self::InvalidInput(_) => "invalid_input",
            Self::Config(_) => "config_error",
            Self::Transient(_) => "transient_error",
            Self::RateLimited(_) => "rate_limited",
            Self::Api { .. } => "api_error",
            Self::Io(_) => "io_error",
            Self::Update(_) => "update_error",
        }
    }

    pub fn suggestion(&self) -> &str {
        match self {
            Self::InvalidInput(_) => "Check arguments with: seedance --help",
            Self::Config(_) => "Check config with: seedance config path",
            Self::Transient(_) | Self::Io(_) => "Retry the command",
            Self::RateLimited(_) => "Wait a moment and retry",
            Self::Api { .. } => {
                "Check status on BytePlus: https://console.byteplus.com/ark -- or run `seedance doctor`"
            }
            Self::Update(_) => "Retry later, or install manually via cargo install seedance",
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() || e.is_connect() || e.is_request() {
            AppError::Transient(e.to_string())
        } else if e.status().is_some_and(|s| s.as_u16() == 429) {
            AppError::RateLimited(e.to_string())
        } else {
            AppError::Transient(e.to_string())
        }
    }
}
