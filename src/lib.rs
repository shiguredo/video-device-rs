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

mod error;
mod frame_math;
mod types;

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod ffi;

#[cfg(target_os = "windows")]
mod capture_mf;
#[cfg(target_os = "windows")]
mod device_mf;

pub use error::{Error, Result};
pub use types::{
    PixelBuffer, PixelFormat, VideoCapture, VideoCaptureConfig, VideoDevice, VideoDeviceList,
    VideoFormat, VideoFrame, VideoFrameOwned,
};

// バックエンド別の具象型を条件付きで公開
#[cfg(all(target_os = "linux", feature = "v4l2"))]
mod device_v4l2;
#[cfg(all(target_os = "linux", feature = "v4l2"))]
pub use device_v4l2::{V4l2VideoDevice, V4l2VideoDeviceList};
#[cfg(all(target_os = "linux", feature = "v4l2"))]
mod capture_v4l2;
#[cfg(all(target_os = "linux", feature = "v4l2"))]
pub use capture_v4l2::V4l2VideoCapture;

#[cfg(all(target_os = "linux", feature = "pipewire"))]
mod device_pipewire;
#[cfg(all(target_os = "linux", feature = "pipewire"))]
pub use device_pipewire::{PipewireVideoDevice, PipewireVideoDeviceList};
#[cfg(all(target_os = "linux", feature = "pipewire"))]
mod capture_pipewire;
#[cfg(all(target_os = "linux", feature = "pipewire"))]
pub use capture_pipewire::PipewireVideoCapture;

#[cfg(target_os = "macos")]
mod device_avf;
#[cfg(target_os = "macos")]
pub use device_avf::{AvfVideoDevice, AvfVideoDeviceList};
#[cfg(target_os = "macos")]
mod capture_avf;
#[cfg(target_os = "macos")]
pub use capture_avf::AvfVideoCapture;

#[cfg(target_os = "windows")]
pub use capture_mf::MfVideoCapture;
#[cfg(target_os = "windows")]
pub use device_mf::{MfVideoDevice, MfVideoDeviceList};
