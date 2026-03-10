use crate::types::PixelFormat;

#[derive(Debug, Clone)]
pub enum Error {
    DeviceNotFound,
    DeviceAccessDenied,
    SessionCreateFailed,
    SessionStartFailed,
    UnsupportedPixelFormat(PixelFormat),
    NullPointer(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::DeviceNotFound => write!(f, "camera device not found"),
            Error::DeviceAccessDenied => write!(f, "camera access denied"),
            Error::SessionCreateFailed => write!(f, "failed to create camera session"),
            Error::SessionStartFailed => write!(f, "failed to start camera session"),
            Error::UnsupportedPixelFormat(format) => {
                write!(f, "unsupported pixel format: {format}")
            }
            Error::NullPointer(name) => write!(f, "null pointer: {}", name),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
