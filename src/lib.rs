//! shiguredo_video_device - macOS/Linux/Windows 対応のビデオライブラリ
//!
//! このクレートは macOS (AVFoundation)、Linux (V4L2)、Windows (Media Foundation) をサポートしています。
//! 現在は映像キャプチャ（カメラ入力）の機能を提供しています。
//!
//! ## フレームコールバック
//!
//! [`VideoCapture::new`](VideoCapture::new) に渡すコールバックは、C から呼ばれる経路を跨ぐため**パニックしてはならない**。
//! 渡される [`VideoFrame`] のスライスが指すメモリは、**そのコールバックの実行中にのみ**有効である。
//! コールバック終了後にデータを保持したい場合は [`VideoFrame::to_owned`] で [`VideoFrameOwned`] にコピーする。

mod error;
mod types;

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod capture;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod device;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod ffi;

#[cfg(target_os = "windows")]
mod capture_windows;
#[cfg(target_os = "windows")]
mod device_windows;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use capture::VideoCapture;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use device::{VideoDevice, VideoDeviceList};

#[cfg(target_os = "windows")]
pub use capture_windows::VideoCapture;
#[cfg(target_os = "windows")]
pub use device_windows::{VideoDevice, VideoDeviceList};

pub use error::{Error, Result};
pub use types::{
    PixelBuffer, PixelFormat, VideoCaptureConfig, VideoFormat, VideoFrame, VideoFrameOwned,
};
