//! ビデオキャプチャの定義。
//!
//! バックエンドに依存しない形でキャプチャの開始・停止・設定取得を提供する
//! [`VideoCapture`] を提供する。

use crate::error::Result;
use crate::types::{VideoCaptureConfig, VideoFrame};

#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
use crate::capture_ffi::FfiCaptureImpl;

#[cfg(enable_mf)]
use crate::capture_mf::MfCaptureImpl;

/// ビデオキャプチャ。
///
/// キャプチャの開始・停止・設定取得を提供する。
pub struct VideoCapture(VideoCaptureInner);

enum VideoCaptureInner {
    /// macOS / Linux の FFI ベースキャプチャ。
    #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
    Ffi(FfiCaptureImpl),
    /// Windows Media Foundation ベースキャプチャ。
    #[cfg(enable_mf)]
    Mf(MfCaptureImpl),
}

impl VideoCapture {
    /// デフォルトバックエンドでキャプチャを構築する。
    ///
    /// この時点ではキャプチャスレッドは起動せず、`start()` が呼ばれるまで待機する。
    /// ビルド時に選択されたデフォルトバックエンドを自動的に使用する。
    #[cfg(any(
        enable_default_avf,
        enable_default_v4l2,
        enable_default_pipewire,
        enable_default_mf
    ))]
    pub fn new<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        #[cfg(enable_default_avf)]
        {
            Self::new_avf(config, callback)
        }
        #[cfg(enable_default_v4l2)]
        {
            Self::new_v4l2(config, callback)
        }
        #[cfg(enable_default_pipewire)]
        {
            Self::new_pipewire(config, callback)
        }
        #[cfg(enable_default_mf)]
        {
            Self::new_mf(config, callback)
        }
    }

    /// macOS AVFoundation でキャプチャを構築する。
    ///
    /// この時点ではキャプチャスレッドは起動せず、`start()` が呼ばれるまで待機する。
    #[cfg(enable_avf)]
    pub fn new_avf<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        Ok(Self(VideoCaptureInner::Ffi(FfiCaptureImpl::new_avf(
            config, callback,
        )?)))
    }

    /// Linux V4L2 でキャプチャを構築する。
    #[cfg(enable_v4l2)]
    pub fn new_v4l2<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        Ok(Self(VideoCaptureInner::Ffi(FfiCaptureImpl::new_v4l2(
            config, callback,
        )?)))
    }

    /// Linux PipeWire でキャプチャを構築する。
    #[cfg(enable_pipewire)]
    pub fn new_pipewire<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        Ok(Self(VideoCaptureInner::Ffi(FfiCaptureImpl::new_pipewire(
            config, callback,
        )?)))
    }

    /// Windows Media Foundation でキャプチャを構築する。
    #[cfg(enable_mf)]
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
    ///
    /// Windows (Media Foundation) でキャプチャスレッドが panic したあとに再 `start` すると
    /// [`crate::Error::CaptureFaulted`] を返す。その場合は新しい [`VideoCapture`] を構築し直すこと。
    pub fn start(&mut self) -> Result<()> {
        match &mut self.0 {
            #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
            VideoCaptureInner::Ffi(inner) => inner.start(),
            #[cfg(enable_mf)]
            VideoCaptureInner::Mf(inner) => inner.start(),
        }
    }

    /// キャプチャを停止する。
    ///
    /// ブロッキング: 全バックエンドでキャプチャスレッド/コールバックの完了を待機してから復帰する。
    /// running でない状態の場合は no-op。
    ///
    /// Windows (Media Foundation) でキャプチャスレッドが panic していた場合、stderr に英語の
    /// エラーログを出力する。その後の再 [`start`](Self::start) は
    /// [`crate::Error::CaptureFaulted`] になる。
    pub fn stop(&mut self) {
        match &mut self.0 {
            #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
            VideoCaptureInner::Ffi(inner) => inner.stop(),
            #[cfg(enable_mf)]
            VideoCaptureInner::Mf(inner) => inner.stop(),
        }
    }

    /// キャプチャ設定を取得する。
    pub fn config(&self) -> &VideoCaptureConfig {
        match &self.0 {
            #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
            VideoCaptureInner::Ffi(inner) => inner.config(),
            #[cfg(enable_mf)]
            VideoCaptureInner::Mf(inner) => inner.config(),
        }
    }
}
