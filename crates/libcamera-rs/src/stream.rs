use std::marker::PhantomData;

use crate::sys as ffi;

use crate::geometry::Size;
use crate::pixel_format::PixelFormat;

/// ストリームロール
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamRole {
    Raw,
    StillCapture,
    VideoRecording,
    Viewfinder,
}

impl StreamRole {
    pub(crate) fn to_raw(self) -> ffi::lc_stream_role_t {
        match self {
            StreamRole::Raw => ffi::lc_stream_role_t_LC_STREAM_ROLE_RAW,
            StreamRole::StillCapture => ffi::lc_stream_role_t_LC_STREAM_ROLE_STILL_CAPTURE,
            StreamRole::VideoRecording => ffi::lc_stream_role_t_LC_STREAM_ROLE_VIDEO_RECORDING,
            StreamRole::Viewfinder => ffi::lc_stream_role_t_LC_STREAM_ROLE_VIEWFINDER,
        }
    }
}

/// ストリーム設定
///
/// CameraConfiguration 内のストリーム設定への参照。
/// ライフタイムで CameraConfiguration の借用を制約する。
pub struct StreamConfiguration<'a> {
    pub(crate) ptr: *mut ffi::lc_stream_configuration_t,
    pub(crate) _phantom: PhantomData<&'a mut ()>,
}

impl<'a> StreamConfiguration<'a> {
    /// ピクセルフォーマットを取得する
    pub fn pixel_format(&self) -> PixelFormat {
        let raw = unsafe { ffi::lc_stream_configuration_get_pixel_format(self.ptr) };
        PixelFormat::from_raw(raw)
    }

    /// ピクセルフォーマットを設定する
    pub fn set_pixel_format(&mut self, fmt: PixelFormat) {
        unsafe { ffi::lc_stream_configuration_set_pixel_format(self.ptr, fmt.to_raw()) };
    }

    /// サイズを取得する
    pub fn size(&self) -> Size {
        let raw = unsafe { ffi::lc_stream_configuration_get_size(self.ptr) };
        Size::from_raw(raw)
    }

    /// サイズを設定する
    pub fn set_size(&mut self, size: Size) {
        unsafe { ffi::lc_stream_configuration_set_size(self.ptr, size.to_raw()) };
    }

    /// ストライドを取得する
    pub fn stride(&self) -> u32 {
        unsafe { ffi::lc_stream_configuration_get_stride(self.ptr) }
    }

    /// フレームサイズを取得する
    pub fn frame_size(&self) -> u32 {
        unsafe { ffi::lc_stream_configuration_get_frame_size(self.ptr) }
    }

    /// バッファ数を取得する
    pub fn buffer_count(&self) -> u32 {
        unsafe { ffi::lc_stream_configuration_get_buffer_count(self.ptr) }
    }

    /// バッファ数を設定する
    pub fn set_buffer_count(&mut self, count: u32) {
        unsafe { ffi::lc_stream_configuration_set_buffer_count(self.ptr, count) };
    }

    /// ストリームを取得する
    pub fn stream(&self) -> Option<Stream> {
        let ptr = unsafe { ffi::lc_stream_configuration_stream(self.ptr) };
        if ptr.is_null() {
            None
        } else {
            Some(Stream { ptr })
        }
    }
}

impl Drop for StreamConfiguration<'_> {
    fn drop(&mut self) {
        unsafe { ffi::lc_stream_configuration_destroy(self.ptr) };
    }
}

/// ストリーム
///
/// Camera 内部データへの不透明参照。所有権は持たない。
#[derive(Debug, Clone)]
pub struct Stream {
    pub(crate) ptr: *mut ffi::lc_stream_t,
}

// Stream は内部ポインタのラッパーであり、libcamera 側でスレッドセーフ
unsafe impl Send for Stream {}
unsafe impl Sync for Stream {}
