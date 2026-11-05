//! このクレートで使用するエラー型の定義。

use std::fmt;

use crate::types::PixelFormat;

/// このクレートのエラー型。
#[derive(Debug, Clone, Copy)]
pub enum Error {
    /// 指定されたカメラデバイスが見つからない
    DeviceNotFound,
    /// カメラデバイスへのアクセスが拒否された
    DeviceAccessDenied,
    /// カメラセッションの作成に失敗した
    SessionCreateFailed,
    /// カメラセッションの開始に失敗した
    SessionStartFailed,
    /// 指定されたピクセルフォーマットに対応していない
    UnsupportedPixelFormat(PixelFormat),
    /// C 側から null ポインタが返された
    ///
    /// フィールドは null だった対象の名前。
    NullPointer(&'static str),
    /// キャプチャ設定がプラットフォーム要件を満たさない
    ///
    /// フィールドは英語の説明メッセージ。
    InvalidCaptureConfig(&'static str),
    /// COM の初期化に失敗した
    ComInitFailed,
    /// C 側が返した文字列が有効な UTF-8 でない
    ///
    /// フィールドは UTF-8 でなかった対象の名前。
    InvalidUtf8(&'static str),
    /// キャプチャスレッドが panic するなどして再 start 不能になった
    CaptureFaulted,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

/// このクレートの処理結果。
pub type Result<T> = std::result::Result<T, Error>;
