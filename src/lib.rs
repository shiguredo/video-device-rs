//! shiguredo_video_device - macOS/Linux/Windows 対応のビデオライブラリ
//!
//! このクレートは macOS (AVFoundation)、Linux (V4L2)、Windows (Media Foundation) をサポートしています。
//! 現在は映像キャプチャ（カメラ入力）の機能を提供しています。
//!
//! ## フレームコールバック
//!
//! [`VideoCapture::start`] を呼ぶときにスレッドが起動し、フレーム毎にコールバックが呼ばれます。
//! 渡される [`VideoFrame`] のスライスが指すメモリは、**そのコールバックの実行中にのみ**有効です。
//! コールバック終了後にデータを保持したい場合は [`VideoFrame::to_owned`] で [`VideoFrameOwned`] にコピーします。
//!
//! ネイティブが未知の FourCC を送った場合、実装によってはユーザーコールバックにフレームが渡らないことがあります（プラットフォーム・デバイスにより異なります）。
//!
//! **キャプチャコールバック内から** `stop` を呼ばないでください。
//! 特に Windows ではキャプチャスレッドが `join` 自身しデッドロックしうるためです。

mod capture;
mod device;
mod error;
mod frame_math;
mod types;

#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
mod capture_ffi;
#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
mod device_ffi;
#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
mod ffi;

#[cfg(enable_mf)]
mod capture_mf;
#[cfg(enable_mf)]
mod device_mf;

pub use capture::VideoCapture;
pub use device::{VideoDevice, VideoDeviceList};
pub use error::{Error, Result};
pub use types::{
    PixelBuffer, PixelFormat, VideoCaptureConfig, VideoFormat, VideoFrame, VideoFrameOwned,
};
