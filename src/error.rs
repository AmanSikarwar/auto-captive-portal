use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Browser error: {0}")]
    Browser(String),

    #[error("Keyring error: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Login failed: {0}")]
    LoginFailed(String),

    #[error("Service error: {0}")]
    Service(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
