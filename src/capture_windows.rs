//! Windows 用ビデオキャプチャ (Media Foundation)

use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use windows::{Win32::Media::MediaFoundation::*, Win32::System::Com::*, core::GUID};

use crate::error::{Error, Result};
use crate::types::{CaptureContext, PixelFormat, VideoCaptureConfig, VideoFrame};

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

pub struct VideoCapture {
    session: Option<SessionData>,
    context: Option<Arc<CaptureContext>>,
    capture_thread: Option<thread::JoinHandle<()>>,
    config: VideoCaptureConfig,
}

impl VideoCapture {
    pub fn new<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + Sync + 'static,
    {
        unsafe {
            // COM 初期化 (既に初期化済みの場合も許容する)
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            // Media Foundation 初期化
            MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_err(|_| Error::SessionCreateFailed)?;

            // デバイスを取得
            let media_source = activate_device(config.device_id.as_deref())?;

            // SourceReader を作成
            let source_reader =
                create_source_reader(&media_source, config.width, config.height, config.fps)?;

            // 設定されたメディアタイプからフォーマット情報を取得
            let (pixel_format, width, height) = get_configured_format(&source_reader)?;

            let context = Arc::new(CaptureContext {
                callback: Box::new(callback),
                running: AtomicBool::new(false),
            });

            let session = SessionData {
                source_reader,
                media_source,
                pixel_format,
                width,
                height,
            };

            Ok(Self {
                session: Some(session),
                context: Some(context),
                capture_thread: None,
                config,
            })
        }
    }

    pub fn start(&mut self) -> Result<()> {
        let session = self.session.as_ref().ok_or(Error::SessionStartFailed)?;
        let context = self.context.as_ref().ok_or(Error::SessionStartFailed)?;

        if context.running.load(Ordering::Acquire) {
            return Ok(());
        }

        context.running.store(true, Ordering::Release);

        // キャプチャに必要なデータをクローン（Send ラッパーで包む）
        let source_reader = SendPtr(session.source_reader.clone());
        let pixel_format = session.pixel_format;
        let width = session.width;
        let height = session.height;
        let context_clone = Arc::clone(context);

        // キャプチャスレッドを開始
        let handle = thread::spawn(move || {
            capture_thread_func(
                source_reader.into_inner(),
                pixel_format,
                width,
                height,
                context_clone,
            );
        });

        self.capture_thread = Some(handle);

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(context) = &self.context
            && context.running.load(Ordering::Acquire)
        {
            context.running.store(false, Ordering::Release);

            // スレッドの終了を待機
            if let Some(handle) = self.capture_thread.take() {
                let _ = handle.join();
            }
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
            unsafe {
                let _ = session.media_source.Shutdown();
            }
        }

        unsafe {
            let _ = MFShutdown();
        }
    }
}

unsafe impl Send for VideoCapture {}
unsafe impl Sync for VideoCapture {}

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

        // デバイス配列を解放
        CoTaskMemFree(Some(devices_ptr as *const _));

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

        // NV12 を試す
        let result = try_set_format(
            &source_reader,
            &media_type,
            &MFVideoFormat_NV12,
            width,
            height,
            fps,
        );
        if result.is_ok() {
            return Ok(source_reader);
        }

        // YUY2 を試す
        let result = try_set_format(
            &source_reader,
            &media_type,
            &MFVideoFormat_YUY2,
            width,
            height,
            fps,
        );
        if result.is_ok() {
            return Ok(source_reader);
        }

        // どちらも失敗した場合はネイティブフォーマットを使用
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

        let pixel_format = if subtype == MFVideoFormat_NV12 {
            PixelFormat::Nv12
        } else if subtype == MFVideoFormat_YUY2 {
            PixelFormat::Yuy2
        } else if subtype == MFVideoFormat_I420 {
            PixelFormat::I420
        } else {
            // サポートされていないフォーマット、NV12 として扱う
            PixelFormat::Nv12
        };

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
    context: Arc<CaptureContext>,
) {
    unsafe {
        // スレッドでも COM 初期化
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        while context.running.load(Ordering::Acquire) {
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
                continue;
            }

            if let Some(sample) = sample {
                // タイムスタンプを 100ns 単位からマイクロ秒に変換
                let timestamp_us = timestamp / 10;

                process_sample(&sample, pixel_format, width, height, timestamp_us, &context);
            }
        }

        CoUninitialize();
    }
}

/// サンプルを処理
unsafe fn process_sample(
    sample: &IMFSample,
    pixel_format: PixelFormat,
    width: i32,
    height: i32,
    timestamp_us: i64,
    context: &CaptureContext,
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

        let data = std::slice::from_raw_parts(data_ptr, current_length as usize);

        let frame = match pixel_format {
            PixelFormat::Nv12 => {
                let y_size = (width * height) as usize;
                if data.len() < y_size {
                    let _ = buffer.Unlock();
                    return;
                }
                let y_data = &data[..y_size];
                let uv_data = &data[y_size..];

                VideoFrame {
                    data: y_data,
                    uv_data: Some(uv_data),
                    width,
                    height,
                    stride: width,
                    stride_uv: width,
                    pixel_format,
                    timestamp_us,
                }
            }
            PixelFormat::I420 => {
                let y_size = (width * height) as usize;
                if data.len() < y_size {
                    let _ = buffer.Unlock();
                    return;
                }
                let y_data = &data[..y_size];
                let uv_data = &data[y_size..];

                VideoFrame {
                    data: y_data,
                    uv_data: Some(uv_data),
                    width,
                    height,
                    stride: width,
                    stride_uv: width / 2,
                    pixel_format,
                    timestamp_us,
                }
            }
            PixelFormat::Yuy2 => {
                let stride = width * 2;
                VideoFrame {
                    data,
                    uv_data: None,
                    width,
                    height,
                    stride,
                    stride_uv: 0,
                    pixel_format,
                    timestamp_us,
                }
            }
            PixelFormat::Unknown(_) => {
                let _ = buffer.Unlock();
                return;
            }
        };

        (context.callback)(frame);

        let _ = buffer.Unlock();
    }
}
