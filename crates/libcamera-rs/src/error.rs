use std::fmt;

/// libcamera 操作のエラー型
#[derive(Debug)]
pub enum Error {
    /// カメラマネージャーの開始に失敗
    StartFailed,
    /// カメラの取得 (acquire) に失敗
    AcquireFailed,
    /// カメラの設定に失敗
    ConfigureFailed,
    /// 設定の生成に失敗
    GenerateConfigFailed,
    /// 設定が無効
    InvalidConfig,
    /// リクエストの作成に失敗
    CreateRequestFailed,
    /// リクエストのキューイングに失敗
    QueueRequestFailed,
    /// カメラの開始に失敗
    CameraStartFailed,
    /// カメラの停止に失敗
    CameraStopFailed,
    /// バッファの割り当てに失敗
    AllocateFailed,
    /// バッファの解放に失敗
    FreeFailed,
    /// リクエストへのバッファ追加に失敗
    AddBufferFailed,
    /// インデックスが範囲外
    IndexOutOfRange,
    /// カメラが見つからない
    CameraNotFound,
    /// コントロール値の取得に失敗
    ControlGetFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::StartFailed => write!(f, "failed to start camera manager"),
            Error::AcquireFailed => write!(f, "failed to acquire camera"),
            Error::ConfigureFailed => write!(f, "failed to configure camera"),
            Error::GenerateConfigFailed => {
                write!(f, "failed to generate configuration")
            }
            Error::InvalidConfig => write!(f, "invalid configuration"),
            Error::CreateRequestFailed => write!(f, "failed to create request"),
            Error::QueueRequestFailed => write!(f, "failed to queue request"),
            Error::CameraStartFailed => write!(f, "failed to start camera"),
            Error::CameraStopFailed => write!(f, "failed to stop camera"),
            Error::AllocateFailed => write!(f, "failed to allocate buffers"),
            Error::FreeFailed => write!(f, "failed to free buffers"),
            Error::AddBufferFailed => {
                write!(f, "failed to add buffer to request")
            }
            Error::IndexOutOfRange => write!(f, "index out of range"),
            Error::CameraNotFound => write!(f, "camera not found"),
            Error::ControlGetFailed => {
                write!(f, "failed to get control value")
            }
        }
    }
}

impl std::error::Error for Error {}

/// libcamera 操作の Result 型エイリアス
pub type Result<T> = std::result::Result<T, Error>;
