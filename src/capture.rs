//! ビデオキャプチャの定義。
//!
//! バックエンドに依存しない形でキャプチャの開始・停止・設定取得を提供する
//! [`VideoCapture`] を提供する。

use crate::error::Result;
use crate::types::{VideoCaptureConfig, VideoFrame};

#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::capture_ffi::FfiCaptureImpl;

#[cfg(target_os = "windows")]
use crate::capture_mf::MfCaptureImpl;

/// ビデオキャプチャ。
///
/// キャプチャの開始・停止・設定取得を提供する。
pub struct VideoCapture(VideoCaptureInner);

enum VideoCaptureInner {
    /// macOS / Linux の FFI ベースキャプチャ。
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Ffi(FfiCaptureImpl),
    /// Windows Media Foundation ベースキャプチャ。
    #[cfg(target_os = "windows")]
    Mf(MfCaptureImpl),
}

impl VideoCapture {
    /// macOS AVFoundation でキャプチャを構築する。
    ///
    /// この時点ではキャプチャスレッドは起動せず、`start()` が呼ばれるまで待機する。
    #[cfg(target_os = "macos")]
    pub fn new_avf<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        Ok(Self(VideoCaptureInner::Ffi(FfiCaptureImpl::new_avf(
            config, callback,
        )?)))
    }

    /// Linux V4L2 でキャプチャを構築する。
    #[cfg(all(target_os = "linux", feature = "v4l2"))]
    pub fn new_v4l2<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        Ok(Self(VideoCaptureInner::Ffi(FfiCaptureImpl::new_v4l2(
            config, callback,
        )?)))
    }

    /// Linux PipeWire でキャプチャを構築する。
    #[cfg(all(target_os = "linux", feature = "pipewire"))]
    pub fn new_pipewire<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        Ok(Self(VideoCaptureInner::Ffi(FfiCaptureImpl::new_pipewire(
            config, callback,
        )?)))
    }

    /// Windows Media Foundation でキャプチャを構築する。
    #[cfg(target_os = "windows")]
    pub fn new_mf<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        Ok(Self(VideoCaptureInner::Mf(MfCaptureImpl::new(
            config, callback,
        )?)))
    }

    /// キャプチャを開始する。
    ///
    /// 冪等: 既に running 状態であれば `Ok(())` を返す。
    /// `stop` 後の再 `start` は許容する。
    ///
    /// PipeWire バックエンドでは内部でストリーミング状態になるまでブロックする。
    /// 他バックエンドでは即座に復帰する。
    pub fn start(&mut self) -> Result<()> {
        match &mut self.0 {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            VideoCaptureInner::Ffi(inner) => inner.start(),
            #[cfg(target_os = "windows")]
            VideoCaptureInner::Mf(inner) => inner.start(),
        }
    }

    /// キャプチャを停止する。
    ///
    /// ブロッキング: 全バックエンドでキャプチャスレッド/コールバックの完了を待機してから復帰する。
    /// running でない状態の場合は no-op。
    pub fn stop(&mut self) {
        match &mut self.0 {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            VideoCaptureInner::Ffi(inner) => inner.stop(),
            #[cfg(target_os = "windows")]
            VideoCaptureInner::Mf(inner) => inner.stop(),
        }
    }

    /// キャプチャ設定を取得する。
    pub fn config(&self) -> &VideoCaptureConfig {
        match &self.0 {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            VideoCaptureInner::Ffi(inner) => inner.config(),
            #[cfg(target_os = "windows")]
            VideoCaptureInner::Mf(inner) => inner.config(),
        }
    }
}
