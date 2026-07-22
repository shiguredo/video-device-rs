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
    /// C 側が返した文字列が有効な UTF-8 でない
    InvalidUtf8(&'static str),
    /// キャプチャスレッドが panic するなどして再 start 不能になった
    CaptureFaulted,
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
            Error::InvalidUtf8(field) => write!(f, "invalid UTF-8 in {}", field),
            Error::CaptureFaulted => {
                write!(
                    f,
                    "capture is faulted and cannot be restarted (capture thread panicked)"
                )
            }
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
