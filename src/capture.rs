use std::ffi::CString;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{Error, Result};
use crate::ffi;
use crate::types::{CaptureContext, PixelBuffer, PixelFormat, VideoCaptureConfig, VideoFrame};

pub struct VideoCapture {
    session: Option<NonNull<ffi::VideoSession>>,
    context: Option<Box<CaptureContext>>,
    config: VideoCaptureConfig,
}

impl VideoCapture {
    pub fn new<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        let requested_pixel_format = match config.pixel_format {
            Some(pixel_format @ PixelFormat::Unknown(_)) => {
                return Err(Error::UnsupportedPixelFormat(pixel_format));
            }
            Some(pixel_format) => pixel_format.to_raw(),
            None => 0,
        };

        let device_id_cstr = config.device_id.as_ref().map(|s| CString::new(s.as_str()));
        let device_id_ptr = match &device_id_cstr {
            Some(Ok(cstr)) => cstr.as_ptr(),
            Some(Err(_)) => return Err(Error::NullPointer("device_id contains null byte")),
            None => std::ptr::null(),
        };

        let session = unsafe {
            ffi::video_session_create(
                device_id_ptr,
                config.width,
                config.height,
                config.fps,
                requested_pixel_format,
            )
        };

        let session = NonNull::new(session).ok_or(Error::SessionCreateFailed)?;

        let context = Box::new(CaptureContext {
            callback: Box::new(callback),
            running: AtomicBool::new(false),
        });

        Ok(Self {
            session: Some(session),
            context: Some(context),
            config,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        let session = self.session.ok_or(Error::SessionStartFailed)?;
        let context = self.context.as_mut().ok_or(Error::SessionStartFailed)?;

        if context.running.load(Ordering::Acquire) {
            return Ok(());
        }

        let context_ptr = context.as_mut() as *mut CaptureContext as *mut std::ffi::c_void;
        let ret = unsafe {
            ffi::video_session_start(session.as_ptr(), Some(frame_callback), context_ptr)
        };

        if ret < 0 {
            return Err(Error::SessionStartFailed);
        }

        context.running.store(true, Ordering::Release);
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(context) = &self.context
            && context.running.load(Ordering::Acquire)
        {
            if let Some(session) = self.session {
                unsafe { ffi::video_session_stop(session.as_ptr()) };
            }
            context.running.store(false, Ordering::Release);
        }
    }

    pub fn config(&self) -> &VideoCaptureConfig {
        &self.config
    }
}

impl Drop for VideoCapture {
    fn drop(&mut self) {
        self.stop();
        if let Some(session) = self.session.take() {
            unsafe { ffi::video_session_destroy(session.as_ptr()) };
        }
    }
}

unsafe impl Send for VideoCapture {}

/// NV12 の Y / UV プレーンのバイト長を計算する。負のストライドやオーバーフロー時は None。
fn nv12_plane_sizes(stride: i32, stride_uv: i32, height: i32) -> Option<(usize, usize)> {
    if stride <= 0 || stride_uv <= 0 || height <= 0 {
        return None;
    }
    let h = height as usize;
    let y = (stride as usize).checked_mul(h)?;
    let uv_h = h.div_ceil(2);
    let uv = (stride_uv as usize).checked_mul(uv_h)?;
    Some((y, uv))
}

/// I420 の Y / 連結 UV（U 行のあと V 行）のバイト長を計算する。
///
/// macOS `video_c.m` は `chromaHeight = (height + 1) / 2`、`uvSize = strideUV * chromaHeight * 2`
/// で `calloc` し、`stride_uv` は `(int)strideUV` としてコールバックに渡す。
/// 本関数の UV は `stride_uv * ((height + 1) / 2) * 2`（usize での切り上げ整合）であり、
/// 偶数 `height` では `stride_uv * height` と同値、奇数 `height` では C の `uvSize` と一致する。
/// Linux PipeWire など他経路の I420 では、連結 UV の実バイト数やストライドの解釈がこの式と一致しない場合がある。
fn i420_plane_sizes(stride: i32, stride_uv: i32, height: i32) -> Option<(usize, usize)> {
    if stride <= 0 || stride_uv <= 0 || height <= 0 {
        return None;
    }
    let h = height as usize;
    let y = (stride as usize).checked_mul(h)?;
    let chroma_h = h.div_ceil(2);
    let uv = (stride_uv as usize).checked_mul(chroma_h)?.checked_mul(2)?;
    Some((y, uv))
}

/// YUY2 の 1 フレーム分のバイト長を計算する。
fn yuy2_packed_frame_bytes(stride: i32, height: i32) -> Option<usize> {
    if stride <= 0 || height <= 0 {
        return None;
    }
    (stride as usize).checked_mul(height as usize)
}

extern "C" fn frame_callback(
    user_data: *mut std::ffi::c_void,
    data: *const u8,
    uv_data: *const u8,
    width: i32,
    height: i32,
    stride: i32,
    stride_uv: i32,
    pixel_format: u32,
    timestamp_us: i64,
    pixel_buffer: *mut std::ffi::c_void,
) {
    let pixel_buffer = unsafe { PixelBuffer::from_retained_ptr(pixel_buffer) };

    if user_data.is_null() || data.is_null() || width <= 0 || height <= 0 {
        return;
    }

    // SAFETY: user_data は Box<CaptureContext> から取得したポインタ
    // context の生存期間は VideoCapture によって保証される
    let context = unsafe { &*(user_data as *const CaptureContext) };

    let pf = PixelFormat::from_raw(pixel_format);

    let frame = match pf {
        PixelFormat::Nv12 => {
            if uv_data.is_null() {
                return;
            }
            let Some((y_size, uv_size)) = nv12_plane_sizes(stride, stride_uv, height) else {
                return;
            };

            let y_slice = unsafe { std::slice::from_raw_parts(data, y_size) };
            let uv_slice = unsafe { std::slice::from_raw_parts(uv_data, uv_size) };

            VideoFrame {
                data: y_slice,
                uv_data: Some(uv_slice),
                width,
                height,
                stride,
                stride_uv,
                pixel_format: pf,
                timestamp_us,
                pixel_buffer,
            }
        }
        PixelFormat::I420 => {
            if uv_data.is_null() {
                return;
            }
            let Some((y_size, uv_size)) = i420_plane_sizes(stride, stride_uv, height) else {
                return;
            };

            let y_slice = unsafe { std::slice::from_raw_parts(data, y_size) };
            let uv_slice = unsafe { std::slice::from_raw_parts(uv_data, uv_size) };

            VideoFrame {
                data: y_slice,
                uv_data: Some(uv_slice),
                width,
                height,
                stride,
                stride_uv,
                pixel_format: pf,
                timestamp_us,
                pixel_buffer,
            }
        }
        PixelFormat::Yuy2 => {
            let Some(data_size) = yuy2_packed_frame_bytes(stride, height) else {
                return;
            };
            let data_slice = unsafe { std::slice::from_raw_parts(data, data_size) };

            VideoFrame {
                data: data_slice,
                uv_data: None,
                width,
                height,
                stride,
                stride_uv: 0,
                pixel_format: pf,
                timestamp_us,
                pixel_buffer,
            }
        }
        PixelFormat::Unknown(_) => return,
    };

    // SAFETY: ユーザコールバックが panic すると extern "C" 境界を越えて
    // unwind し未定義動作になるため、catch_unwind で防ぐ
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        (context.callback)(frame);
    }));
}

#[cfg(test)]
mod tests {
    use super::{i420_plane_sizes, nv12_plane_sizes, yuy2_packed_frame_bytes};

    #[test]
    fn nv12_rejects_non_positive_dimensions() {
        assert_eq!(nv12_plane_sizes(0, 4, 480), None);
        assert_eq!(nv12_plane_sizes(4, 0, 480), None);
        assert_eq!(nv12_plane_sizes(4, 4, 0), None);
        assert_eq!(nv12_plane_sizes(-1, 4, 480), None);
    }

    #[test]
    fn nv12_small_known_sizes() {
        // Y: 4*2=8, UV 行は div_ceil(2,2)=1, UV: 4*1=4
        assert_eq!(nv12_plane_sizes(4, 4, 2), Some((8, 4)));
    }

    #[test]
    fn nv12_odd_height_uv_rows_use_div_ceil() {
        // height=3 -> uv 行数 2, UV: stride_uv * 2
        assert_eq!(nv12_plane_sizes(8, 8, 3), Some((24, 16)));
    }

    #[test]
    fn i420_matches_macos_uv_formula() {
        // video_c.m: uvSize = strideUV * chromaHeight * 2, chromaHeight = (height + 1) / 2
        // height=480, stride_uv=320 -> chroma_h=240, uv=320*240*2=153600
        assert_eq!(i420_plane_sizes(640, 320, 480), Some((307_200, 153_600)));
        // 奇数 height=3, stride_uv=4 -> chroma_h=2, uv=4*2*2=16
        assert_eq!(i420_plane_sizes(8, 4, 3), Some((24, 16)));
    }

    #[test]
    fn i420_rejects_non_positive() {
        assert_eq!(i420_plane_sizes(0, 4, 100), None);
        assert_eq!(i420_plane_sizes(4, 0, 100), None);
        assert_eq!(i420_plane_sizes(4, 4, -1), None);
    }

    #[test]
    fn yuy2_packed_bytes_stride_times_height() {
        assert_eq!(yuy2_packed_frame_bytes(640, 480), Some(307_200));
        assert_eq!(yuy2_packed_frame_bytes(0, 480), None);
        assert_eq!(yuy2_packed_frame_bytes(640, 0), None);
    }
}
