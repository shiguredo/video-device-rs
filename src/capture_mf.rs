//! Windows 用ビデオキャプチャ (Media Foundation)

use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use windows::{Win32::Media::MediaFoundation::*, core::GUID};

use crate::VideoCapture;
use crate::error::{Error, Result};
use crate::frame_math;
use crate::types::{
    CoInitGuard, CoTaskMemActivateArrayGuard, PixelFormat, VideoCaptureConfig, VideoFrame,
    guid_to_pixel_format, pixel_format_to_guid,
};

/// Send でない型をスレッドに渡すためのラッパー（MTA で初期化済みのため安全）
struct SendPtr<T>(T);
unsafe impl<T> Send for SendPtr<T> {}
impl<T> SendPtr<T> {
    fn into_inner(self) -> T {
        self.0
    }
}

struct SessionData {
    source_reader: IMFSourceReader,
    media_source: IMFMediaSource,
    pixel_format: PixelFormat,
    width: i32,
    height: i32,
}

type VideoFrameCallback = Box<dyn Fn(VideoFrame<'_>) + Send + 'static>;

/// Windows 用ビデオキャプチャ (Media Foundation)。
///
/// [`MfVideoCapture::stop`] をフレームコールバック内から呼ばないこと（キャプチャスレッドが自身を `join` しデッドロックしうる）。
pub struct MfVideoCapture {
    session: Option<SessionData>,
    running: Arc<AtomicBool>,
    callback: Option<VideoFrameCallback>,
    capture_thread: Option<thread::JoinHandle<VideoFrameCallback>>,
    config: VideoCaptureConfig,
    _com_guard: CoInitGuard,
}

fn validate_capture_config_for_windows(config: &VideoCaptureConfig) -> Result<()> {
    // Media Foundation へ幅・高さ・fps を渡すとき `as u64` で属性に詰めるため、0 以下や負の i32 は
    // 意図した解像度・フレームレートにならない。先に拒否する。
    if config.width <= 0 || config.height <= 0 || config.fps <= 0 {
        return Err(Error::InvalidCaptureConfig(
            "width, height, and fps must be positive integers on Windows",
        ));
    }
    Ok(())
}

impl VideoCapture for MfVideoCapture {
    fn start(&mut self) -> Result<()> {
        let session = self.session.as_ref().ok_or(Error::SessionStartFailed)?;
        let callback = self.callback.take();

        if callback.is_none() || self.running.load(Ordering::Acquire) {
            return Ok(());
        }

        self.running.store(true, Ordering::Release);

        // キャプチャに必要なデータをクローン（Send ラッパーで包む）
        let source_reader = SendPtr(session.source_reader.clone());
        let pixel_format = session.pixel_format;
        let width = session.width;
        let height = session.height;
        let running_clone = Arc::clone(&self.running);

        // キャプチャスレッドを開始
        // 戻り値型を VideoFrameCallback にすることで、stop() 時にコールバックを回収できる
        let handle = thread::spawn(move || -> VideoFrameCallback {
            capture_thread_func(
                source_reader.into_inner(),
                pixel_format,
                width,
                height,
                running_clone,
                callback.unwrap(),
            )
        });

        self.capture_thread = Some(handle);

        Ok(())
    }

    fn stop(&mut self) {
        if self.running.load(Ordering::Acquire) {
            self.running.store(false, Ordering::Release);

            // スレッドの終了を待機し、コールバックを回収する
            if let Some(handle) = self.capture_thread.take()
                && let Ok(callback) = handle.join()
            {
                self.callback = Some(callback);
            }
        }
    }

    fn config(&self) -> &VideoCaptureConfig {
        &self.config
    }
}

impl MfVideoCapture {
    pub fn new<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        validate_capture_config_for_windows(&config)?;

        unsafe {
            let com_guard = CoInitGuard::new()?;

            // Media Foundation 初期化（失敗時は即座にエラーを返す）
            MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_err(|_| Error::SessionCreateFailed)?;
            let result = {
                // デバイスを取得
                let media_source = activate_device(config.device_id.as_deref())?;

                // SourceReader を作成
                let source_reader = create_source_reader(
                    &media_source,
                    config.width,
                    config.height,
                    config.fps,
                    config.pixel_format,
                )?;

                // 設定されたメディアタイプからフォーマット情報を取得
                let (pixel_format, width, height) = get_configured_format(&source_reader)?;
                if matches!(pixel_format, PixelFormat::Unknown(_)) {
                    return Err(Error::UnsupportedPixelFormat(pixel_format));
                }

                let running = Arc::new(AtomicBool::new(false));

                let session = SessionData {
                    source_reader,
                    media_source,
                    pixel_format,
                    width,
                    height,
                };

                Ok(Self {
                    session: Some(session),
                    running,
                    callback: Some(Box::new(callback)),
                    capture_thread: None,
                    config,
                    _com_guard: com_guard,
                })
            };

            // 構築途中で失敗した場合は MFStartup とつりあわせるために MFShutdown を呼ぶ
            if result.is_err() {
                let _ = MFShutdown();
            }

            result
        }
    }
}

impl Drop for MfVideoCapture {
    fn drop(&mut self) {
        self.stop();

        if let Some(session) = self.session.take() {
            unsafe {
                let _ = session.media_source.Shutdown();
            }
        }

        unsafe {
            let _ = MFShutdown();
        }
    }
}

/// デバイスをアクティベート
unsafe fn activate_device(device_id: Option<&str>) -> Result<IMFMediaSource> {
    unsafe {
        // デバイス列挙用の属性を作成
        let mut attributes: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attributes, 1).map_err(|_| Error::SessionCreateFailed)?;
        let attributes = attributes.ok_or(Error::SessionCreateFailed)?;

        attributes
            .SetGUID(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            )
            .map_err(|_| Error::SessionCreateFailed)?;

        // デバイスを列挙
        let mut devices_ptr: *mut Option<IMFActivate> = ptr::null_mut();
        let mut count: u32 = 0;

        MFEnumDeviceSources(&attributes, &mut devices_ptr, &mut count)
            .map_err(|_| Error::DeviceAccessDenied)?;

        if count == 0 || devices_ptr.is_null() {
            return Err(Error::DeviceNotFound);
        }

        let _devices_guard = CoTaskMemActivateArrayGuard {
            ptr: devices_ptr,
            count,
        };

        let device_slice = std::slice::from_raw_parts(devices_ptr, count as usize);

        // 指定されたデバイスまたはデフォルトデバイスを選択
        let activate = if let Some(target_id) = device_id {
            device_slice
                .iter()
                .find(|opt| {
                    if let Some(act) = opt
                        && let Some(id) = get_device_symbolic_link(act)
                    {
                        return id == target_id;
                    }
                    false
                })
                .and_then(|opt| opt.as_ref())
                .ok_or(Error::DeviceNotFound)?
        } else {
            // デフォルトは最初のデバイス
            device_slice
                .first()
                .and_then(|opt| opt.as_ref())
                .ok_or(Error::DeviceNotFound)?
        };

        // メディアソースをアクティベート
        let source: IMFMediaSource = activate
            .ActivateObject()
            .map_err(|_| Error::SessionCreateFailed)?;

        Ok(source)
    }
}

/// デバイスからシンボリックリンクを取得
fn get_device_symbolic_link(activate: &IMFActivate) -> Option<String> {
    unsafe {
        let key = &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK;
        let length = activate.GetStringLength(key).ok()?;
        if length == 0 {
            return None;
        }

        let mut buffer: Vec<u16> = vec![0; (length + 1) as usize];
        activate.GetString(key, &mut buffer, None).ok()?;

        String::from_utf16(&buffer[..length as usize]).ok()
    }
}

/// SourceReader を作成してメディアタイプを設定
unsafe fn create_source_reader(
    media_source: &IMFMediaSource,
    width: i32,
    height: i32,
    fps: i32,
    requested_pixel_format: Option<PixelFormat>,
) -> Result<IMFSourceReader> {
    unsafe {
        // SourceReader を作成
        let source_reader: IMFSourceReader =
            MFCreateSourceReaderFromMediaSource(media_source, None)
                .map_err(|_| Error::SessionCreateFailed)?;

        // 出力メディアタイプを設定（NV12 優先）
        let media_type: IMFMediaType =
            MFCreateMediaType().map_err(|_| Error::SessionCreateFailed)?;

        media_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(|_| Error::SessionCreateFailed)?;

        if let Some(pixel_format) = requested_pixel_format {
            let subtype = pixel_format_to_guid(pixel_format)
                .ok_or(Error::UnsupportedPixelFormat(pixel_format))?;
            try_set_format(&source_reader, &media_type, &subtype, width, height, fps)
                .map_err(|_| Error::UnsupportedPixelFormat(pixel_format))?;
            return Ok(source_reader);
        }

        for subtype in [MFVideoFormat_NV12, MFVideoFormat_YUY2, MFVideoFormat_I420] {
            if try_set_format(&source_reader, &media_type, &subtype, width, height, fps).is_ok() {
                return Ok(source_reader);
            }
        }

        // どの指定も失敗した場合はネイティブフォーマットを使用
        Ok(source_reader)
    }
}

/// 指定されたフォーマットを設定
unsafe fn try_set_format(
    source_reader: &IMFSourceReader,
    media_type: &IMFMediaType,
    subtype: &GUID,
    width: i32,
    height: i32,
    fps: i32,
) -> Result<()> {
    unsafe {
        media_type
            .SetGUID(&MF_MT_SUBTYPE, subtype)
            .map_err(|_| Error::SessionCreateFailed)?;

        // フレームサイズを設定
        let frame_size = ((width as u64) << 32) | (height as u64);
        media_type
            .SetUINT64(&MF_MT_FRAME_SIZE, frame_size)
            .map_err(|_| Error::SessionCreateFailed)?;

        // フレームレートを設定
        let frame_rate = ((fps as u64) << 32) | 1u64;
        media_type
            .SetUINT64(&MF_MT_FRAME_RATE, frame_rate)
            .map_err(|_| Error::SessionCreateFailed)?;

        source_reader
            .SetCurrentMediaType(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                None,
                media_type,
            )
            .map_err(|_| Error::SessionCreateFailed)?;

        Ok(())
    }
}

/// 設定されたフォーマット情報を取得
unsafe fn get_configured_format(
    source_reader: &IMFSourceReader,
) -> Result<(PixelFormat, i32, i32)> {
    unsafe {
        let media_type: IMFMediaType = source_reader
            .GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32)
            .map_err(|_| Error::SessionCreateFailed)?;

        // サブタイプを取得
        let subtype: GUID = media_type
            .GetGUID(&MF_MT_SUBTYPE)
            .map_err(|_| Error::SessionCreateFailed)?;

        let pixel_format = guid_to_pixel_format(&subtype).ok_or(Error::SessionCreateFailed)?;

        // フレームサイズを取得
        let frame_size: u64 = media_type
            .GetUINT64(&MF_MT_FRAME_SIZE)
            .map_err(|_| Error::SessionCreateFailed)?;

        let width = (frame_size >> 32) as i32;
        let height = (frame_size & 0xFFFFFFFF) as i32;

        Ok((pixel_format, width, height))
    }
}

/// キャプチャスレッド関数
fn capture_thread_func(
    source_reader: IMFSourceReader,
    pixel_format: PixelFormat,
    width: i32,
    height: i32,
    running: Arc<AtomicBool>,
    callback: VideoFrameCallback,
) -> VideoFrameCallback {
    let _com_guard = match CoInitGuard::new() {
        Ok(g) => g,
        Err(_) => return callback,
    };

    unsafe {
        while running.load(Ordering::Acquire) {
            let mut flags: u32 = 0;
            let mut timestamp: i64 = 0;
            let mut sample: Option<IMFSample> = None;

            let result = source_reader.ReadSample(
                MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32,
                0,
                None,
                Some(&mut flags),
                Some(&mut timestamp),
                Some(&mut sample),
            );

            if result.is_err() {
                // ReadSample 連続失敗で CPU を占有しない
                thread::sleep(Duration::from_millis(1));
                continue;
            }

            if let Some(sample) = sample {
                // タイムスタンプを 100ns 単位からマイクロ秒に変換
                let timestamp_us = timestamp / 10;

                process_sample(
                    &sample,
                    pixel_format,
                    width,
                    height,
                    timestamp_us,
                    &callback,
                );
            }
        }
    }

    callback
}

/// サンプルを処理
unsafe fn process_sample(
    sample: &IMFSample,
    pixel_format: PixelFormat,
    width: i32,
    height: i32,
    timestamp_us: i64,
    callback: &VideoFrameCallback,
) {
    unsafe {
        // バッファを取得
        let buffer: IMFMediaBuffer = match sample.ConvertToContiguousBuffer() {
            Ok(b) => b,
            Err(_) => return,
        };

        let mut data_ptr: *mut u8 = ptr::null_mut();
        let mut max_length: u32 = 0;
        let mut current_length: u32 = 0;

        if buffer
            .Lock(
                &mut data_ptr,
                Some(&mut max_length),
                Some(&mut current_length),
            )
            .is_err()
        {
            return;
        }

        if data_ptr.is_null() {
            let _ = buffer.Unlock();
            return;
        }

        if width <= 0 || height <= 0 {
            let _ = buffer.Unlock();
            return;
        }

        let data = std::slice::from_raw_parts(data_ptr, current_length as usize);

        let frame = match pixel_format {
            PixelFormat::Nv12 => {
                let Some(required) = frame_math::nv12_packed_frame_bytes(width, height) else {
                    let _ = buffer.Unlock();
                    return;
                };
                if data.len() < required {
                    let _ = buffer.Unlock();
                    return;
                }
                let Some(y_size) = (width as usize).checked_mul(height as usize) else {
                    let _ = buffer.Unlock();
                    return;
                };
                let y_data = &data[..y_size];
                let uv_data = &data[y_size..required];

                VideoFrame {
                    data: y_data,
                    uv_data: Some(uv_data),
                    width,
                    height,
                    stride: width,
                    stride_uv: width,
                    pixel_format,
                    timestamp_us,
                    pixel_buffer: None,
                }
            }
            PixelFormat::I420 => {
                let Some(required) = frame_math::i420_packed_frame_bytes(width, height) else {
                    let _ = buffer.Unlock();
                    return;
                };
                if data.len() < required {
                    let _ = buffer.Unlock();
                    return;
                }
                let Some(y_size) = (width as usize).checked_mul(height as usize) else {
                    let _ = buffer.Unlock();
                    return;
                };
                let y_data = &data[..y_size];
                let uv_data = &data[y_size..required];

                VideoFrame {
                    data: y_data,
                    uv_data: Some(uv_data),
                    width,
                    height,
                    stride: width,
                    stride_uv: (width + 1) / 2,
                    pixel_format,
                    timestamp_us,
                    pixel_buffer: None,
                }
            }
            PixelFormat::Yuy2 => {
                let Some(required) = frame_math::yuy2_packed_frame_bytes_win(width, height) else {
                    let _ = buffer.Unlock();
                    return;
                };
                if data.len() < required {
                    let _ = buffer.Unlock();
                    return;
                }
                let Some(stride) = width.checked_mul(2) else {
                    let _ = buffer.Unlock();
                    return;
                };
                VideoFrame {
                    data: &data[..required],
                    uv_data: None,
                    width,
                    height,
                    stride,
                    stride_uv: 0,
                    pixel_format,
                    timestamp_us,
                    pixel_buffer: None,
                }
            }
            PixelFormat::Unknown(_) => {
                let _ = buffer.Unlock();
                return;
            }
        };

        // ユーザコールバックが panic しても Unlock を確実に実行する
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            callback(frame);
        }));

        let _ = buffer.Unlock();
    }
}
