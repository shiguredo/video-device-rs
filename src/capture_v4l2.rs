use std::ffi::CString;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::VideoCapture;
use crate::error::{Error, Result};
use crate::ffi;
use crate::frame_math;
use crate::types::{PixelBuffer, PixelFormat, VideoCaptureConfig, VideoFrame};

struct CaptureContext {
    callback: Box<dyn Fn(VideoFrame<'_>) + Send + 'static>,
    running: AtomicBool,
}

pub struct V4l2VideoCapture {
    session: Option<NonNull<ffi::v4l2::VideoSession>>,
    context: Option<Box<CaptureContext>>,
    config: VideoCaptureConfig,
}

impl VideoCapture for V4l2VideoCapture {
    fn start(&mut self) -> Result<()> {
        let session = self.session.ok_or(Error::SessionStartFailed)?;
        let context = self.context.as_mut().ok_or(Error::SessionStartFailed)?;

        if context.running.load(Ordering::Acquire) {
            return Ok(());
        }

        let context_ptr = &mut **context as *mut CaptureContext as *mut std::ffi::c_void;
        let ret = unsafe {
            ffi::v4l2::video_v4l2_session_start(session.as_ptr(), Some(frame_callback), context_ptr)
        };

        if ret < 0 {
            return Err(Error::SessionStartFailed);
        }

        context.running.store(true, Ordering::Release);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(context) = &self.context
            && context.running.load(Ordering::Acquire)
        {
            if let Some(session) = self.session {
                unsafe { ffi::v4l2::video_v4l2_session_stop(session.as_ptr()) };
            }
            context.running.store(false, Ordering::Release);
        }
    }

    fn config(&self) -> &VideoCaptureConfig {
        &self.config
    }
}

impl V4l2VideoCapture {
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
            ffi::v4l2::video_v4l2_session_create(
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
}

impl Drop for V4l2VideoCapture {
    fn drop(&mut self) {
        self.stop();
        if let Some(session) = self.session.take() {
            unsafe { ffi::v4l2::video_v4l2_session_destroy(session.as_ptr()) };
        }
    }
}

unsafe impl Send for V4l2VideoCapture {}

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

    let context = unsafe { &*(user_data as *const CaptureContext) };

    let pf = PixelFormat::from_raw(pixel_format);

    let frame = match pf {
        PixelFormat::Nv12 => {
            if uv_data.is_null() {
                return;
            }
            let Some((y_size, uv_size)) = frame_math::nv12_plane_sizes(stride, stride_uv, height)
            else {
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
            let Some((y_size, uv_size)) = frame_math::i420_plane_sizes(stride, stride_uv, height)
            else {
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
            let Some(data_size) = frame_math::yuy2_packed_frame_bytes(stride, height) else {
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

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        (context.callback)(frame);
    }));
}
