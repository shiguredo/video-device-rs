use std::ffi::CString;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{Error, Result};
use crate::ffi;
use crate::types::{CaptureContext, PixelFormat, VideoCaptureConfig, VideoFrame};

pub struct VideoCapture {
    session: Option<NonNull<ffi::VideoSession>>,
    context: Option<Arc<CaptureContext>>,
    config: VideoCaptureConfig,
}

impl VideoCapture {
    pub fn new<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + Sync + 'static,
    {
        let device_id_cstr = config.device_id.as_ref().map(|s| CString::new(s.as_str()));
        let device_id_ptr = match &device_id_cstr {
            Some(Ok(cstr)) => cstr.as_ptr(),
            Some(Err(_)) => return Err(Error::NullPointer("device_id contains null byte")),
            None => std::ptr::null(),
        };

        let session = unsafe {
            ffi::video_session_create(device_id_ptr, config.width, config.height, config.fps)
        };

        let session = NonNull::new(session).ok_or(Error::SessionCreateFailed)?;

        let context = Arc::new(CaptureContext {
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
        let context = self.context.as_ref().ok_or(Error::SessionStartFailed)?;

        if context.running.load(Ordering::Acquire) {
            return Ok(());
        }

        let context_ptr = Arc::as_ptr(context) as *mut std::ffi::c_void;
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

// VideoCapture はプラットフォーム固有のキャプチャセッションを内部で管理し、
// コールバックはスレッドセーフな Arc<CaptureContext> を通じて処理される
unsafe impl Send for VideoCapture {}
unsafe impl Sync for VideoCapture {}

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
) {
    if user_data.is_null() || data.is_null() || width <= 0 || height <= 0 {
        return;
    }

    // SAFETY: user_data は Arc<CaptureContext> から取得したポインタ
    // context の生存期間は VideoCapture によって保証される
    let context = unsafe { &*(user_data as *const CaptureContext) };

    let pf = PixelFormat::from_raw(pixel_format);

    let frame = match pf {
        PixelFormat::Nv12 => {
            if uv_data.is_null() {
                return;
            }
            let y_size = (stride as usize) * (height as usize);
            let uv_size = (stride_uv as usize) * (height as usize).div_ceil(2);

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
            }
        }
        PixelFormat::I420 => {
            if uv_data.is_null() {
                return;
            }
            let y_size = (stride as usize) * (height as usize);
            // I420: U プレーン (stride_uv * height/2) + V プレーン (stride_uv * height/2)
            let uv_size = (stride_uv as usize) * (height as usize);

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
            }
        }
        PixelFormat::Yuy2 => {
            let data_size = (stride as usize) * (height as usize);
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
            }
        }
        PixelFormat::Unknown(_) => return,
    };

    (context.callback)(frame);
}
