use crate::types::PixelFormat;

#[derive(Debug, Clone)]
pub enum Error {
    DeviceNotFound,
    DeviceAccessDenied,
    SessionCreateFailed,
    SessionStartFailed,
    UnsupportedPixelFormat(PixelFormat),
    NullPointer(&'static str),
    /// キャプチャ設定がプラットフォーム要件を満たさない（メッセージは英語）
    InvalidCaptureConfig(&'static str),
    /// COM 初期化に失敗
    ComInitFailed,
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
            Error::InvalidCaptureConfig(msg) => write!(f, "invalid capture config: {}", msg),
            Error::ComInitFailed => write!(f, "COM initialization failed"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
