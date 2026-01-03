/// Error types for ADB operations
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AdbError {
    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error("Connection timeout: {0}")]
    Timeout(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("App not found: {0}")]
    AppNotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),
}

pub type Result<T> = std::result::Result<T, AdbError>;
