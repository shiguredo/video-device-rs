//! Linux PipeWire 用のビデオキャプチャ。

use crate::error::Result;
use crate::ffi;
use crate::types::{VideoCaptureConfig, VideoFrame};
use crate::capture_common::{CaptureInner, CaptureOps};

/// PipeWire の FFI 関数テーブル。
const OPS: CaptureOps = CaptureOps {
    session_create:  ffi::video_pipewire_session_create,
    session_start:   ffi::video_pipewire_session_start,
    session_stop:    ffi::video_pipewire_session_stop,
    session_destroy: ffi::video_pipewire_session_destroy,
};

/// Linux PipeWire ビデオキャプチャ。
pub struct PipewireVideoCapture {
    inner: CaptureInner,
}

impl PipewireVideoCapture {
    pub fn new<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        Ok(Self {
            inner: CaptureInner::new(&OPS, config, callback)?,
        })
    }
}

impl crate::VideoCapture for PipewireVideoCapture {
    fn start(&mut self) -> Result<()> {
        self.inner.start()
    }

    fn stop(&mut self) {
        self.inner.stop();
    }

    fn config(&self) -> &VideoCaptureConfig {
        self.inner.config()
    }
}
