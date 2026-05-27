//! macOS AVFoundation 用のビデオキャプチャ。

use crate::error::Result;
use crate::ffi;
use crate::types::{VideoCaptureConfig, VideoFrame};
use crate::capture_common::{CaptureInner, CaptureOps};

/// AVFoundation の FFI 関数テーブル。
const OPS: CaptureOps = CaptureOps {
    session_create:  ffi::video_avf_session_create,
    session_start:   ffi::video_avf_session_start,
    session_stop:    ffi::video_avf_session_stop,
    session_destroy: ffi::video_avf_session_destroy,
};

/// macOS AVFoundation ビデオキャプチャ。
pub struct AvfVideoCapture {
    inner: CaptureInner,
}

impl AvfVideoCapture {
    pub fn new<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        Ok(Self {
            inner: CaptureInner::new(&OPS, config, callback)?,
        })
    }
}

impl crate::VideoCapture for AvfVideoCapture {
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
