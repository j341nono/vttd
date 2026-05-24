use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("config parse error: {0}")]
    ConfigParse(#[from] toml::de::Error),

    #[error("config serialize error: {0}")]
    ConfigSer(#[from] toml::ser::Error),

    #[error("date parse error: {0}")]
    DateParse(#[from] chrono::ParseError),

    #[error("invalid command: {0}")]
    InvalidCommand(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("{0}")]
    Other(String),
}

pub type AppResult<T> = std::result::Result<T, AppError>;
