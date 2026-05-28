//! Linux V4L2 用のビデオキャプチャ。

use crate::capture_ffi::{CaptureInner, CaptureOps};
use crate::error::Result;
use crate::ffi;
use crate::types::{VideoCaptureConfig, VideoFrame};

/// V4L2 の FFI 関数テーブル。
const OPS: CaptureOps = CaptureOps {
    session_create: ffi::video_v4l2_session_create,
    session_start: ffi::video_v4l2_session_start,
    session_stop: ffi::video_v4l2_session_stop,
    session_destroy: ffi::video_v4l2_session_destroy,
};

/// Linux V4L2 ビデオキャプチャ。
pub struct V4l2VideoCapture {
    inner: CaptureInner,
}

impl V4l2VideoCapture {
    pub fn new<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        Ok(Self {
            inner: CaptureInner::new(&OPS, config, callback)?,
        })
    }
}

impl crate::VideoCapture for V4l2VideoCapture {
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
