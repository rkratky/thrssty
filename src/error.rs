use thiserror::Error;

/// Application-level errors for thrssty.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Config parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("Config serialize error: {0}")]
    ConfigSerialize(#[from] toml::ser::Error),

    #[error("Cache error: {0}")]
    Cache(#[from] serde_json::Error),

    #[error("Feed fetch error for '{url}': {reason}")]
    FeedFetch { url: String, reason: String },

    #[error("Theme error in '{file}': {reason}")]
    ThemeParse { file: String, reason: String },

    #[error("Feeds file error in '{file}': {reason}")]
    FeedsFileParse { file: String, reason: String },

    #[error("Config directory not found")]
    MissingConfigDir,

    #[error("Data directory not found")]
    MissingDataDir,
}

/// Convenience Result type using AppError.
pub type Result<T> = std::result::Result<T, AppError>;
