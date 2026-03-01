use std::fmt;

/// Simplified error types for better maintainability
#[derive(Debug)]
pub enum AppError {
    /// Configuration validation errors
    Config {
        field: String,
        value: Option<String>,
        reason: String,
    },

    /// Wallet generation and derivation errors
    Wallet {
        message: String,
        details: Option<String>,
    },

    /// Network and API communication errors
    Network {
        source: Box<dyn std::error::Error + Send + Sync>,
        url: Option<String>,
        retry_count: Option<u32>,
    },

    /// Validation errors for user inputs
    Validation {
        field: String,
        value: String,
        reason: String,
    },

    /// Generic I/O errors
    Io {
        source: std::io::Error,
        operation: String,
    },
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Config { field, value, reason } => {
                write!(f, "Configuration error for '{}': {}", field, reason)?;
                if let Some(val) = value {
                    write!(f, " (value: '{}')", val)?;
                }
                Ok(())
            }
            AppError::Wallet { message, details } => {
                write!(f, "Wallet error: {}", message)?;
                if let Some(d) = details {
                    write!(f, " ({})", d)?;
                }
                Ok(())
            }
            AppError::Network { source, url, retry_count } => {
                write!(f, "Network error: {}", source)?;
                if let Some(u) = url {
                    write!(f, " (URL: {})", u)?;
                }
                if let Some(count) = retry_count {
                    if *count > 0 {
                        write!(f, " (retries: {})", count)?;
                    }
                }
                Ok(())
            }
            AppError::Validation { field, value, reason } => {
                write!(
                    f,
                    "Validation error for '{}': {} (value: '{}')",
                    field, reason, value
                )
            }
            AppError::Io { source, operation } => {
                write!(f, "IO error during {}: {}", operation, source)
            }
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Network { source, .. } => Some(source.as_ref()),
            AppError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

// Conversion implementations for cleaner error handling
impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Network {
            source: Box::new(err),
            url: None,
            retry_count: Some(0),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Network {
            source: Box::new(err),
            url: None,
            retry_count: None,
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io {
            source: err,
            operation: "unknown".to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
