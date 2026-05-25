//! Windows 用ビデオデバイス列挙 (Media Foundation)

use std::ptr;

use windows::{Win32::Media::MediaFoundation::*, Win32::System::Com::*, core::GUID};

use crate::error::{Error, Result};
use crate::types::{PixelFormat, VideoFormat, CoInitGuard};

/// `MFEnumDeviceSources` が返した `IMFActivate` 配列を必ず `CoTaskMemFree` する。
struct CoTaskMemActivateArrayGuard {
    ptr: *mut Option<IMFActivate>,
}

impl Drop for CoTaskMemActivateArrayGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                CoTaskMemFree(Some(self.ptr as *const _));
            }
        }
    }
}

/// ビデオデバイス
pub struct VideoDevice {
    name: String,
    unique_id: String,
    formats: Vec<VideoFormat>,
}

impl VideoDevice {
    /// デバイス名を取得
    pub fn name(&self) -> Result<String> {
        Ok(self.name.clone())
    }

    /// デバイスの一意識別子を取得
    pub fn unique_id(&self) -> Result<String> {
        Ok(self.unique_id.clone())
    }

    /// 対応フォーマット数を取得
    pub fn format_count(&self) -> usize {
        self.formats.len()
    }

    /// 対応フォーマット一覧を取得
    pub fn formats(&self) -> Vec<VideoFormat> {
        self.formats.clone()
    }
}

/// ビデオデバイスリスト
pub struct VideoDeviceList {
    devices: Vec<VideoDevice>,
}

impl VideoDeviceList {
    /// デバイスを列挙
    pub fn enumerate() -> Result<Self> {
        let devices = enumerate_devices_internal()?;
        Ok(Self { devices })
    }

    /// デバイスのスライスを取得
    pub fn devices(&self) -> &[VideoDevice] {
        &self.devices
    }

    /// デバイス数を取得
    pub fn len(&self) -> usize {
        self.devices.len()
    }

    /// デバイスが空かどうか
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }
}

impl<'a> IntoIterator for &'a VideoDeviceList {
    type Item = &'a VideoDevice;
    type IntoIter = std::slice::Iter<'a, VideoDevice>;

    fn into_iter(self) -> Self::IntoIter {
        self.devices.iter()
    }
}

/// Media Foundation GUID を PixelFormat に変換
fn guid_to_pixel_format(guid: &GUID) -> Option<PixelFormat> {
    if *guid == MFVideoFormat_NV12 {
        Some(PixelFormat::Nv12)
    } else if *guid == MFVideoFormat_YUY2 {
        Some(PixelFormat::Yuy2)
    } else if *guid == MFVideoFormat_I420 {
        Some(PixelFormat::I420)
    } else {
        None
    }
}

/// デバイスからフォーマット情報を取得
fn get_device_formats(activate: &IMFActivate) -> Vec<VideoFormat> {
    let mut formats = Vec::new();

    unsafe {
        // メディアソースをアクティベート
        let source: IMFMediaSource = match activate.ActivateObject() {
            Ok(s) => s,
            Err(_) => return formats,
        };

        // ソースリーダーを作成
        let reader: IMFSourceReader = match MFCreateSourceReaderFromMediaSource(&source, None) {
            Ok(r) => r,
            Err(_) => {
                let _ = source.Shutdown();
                return formats;
            }
        };

        // 利用可能なメディアタイプを列挙
        let mut index = 0u32;
        loop {
            let media_type: IMFMediaType = match reader
                .GetNativeMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, index)
            {
                Ok(mt) => mt,
                Err(_) => break,
            };

            // サブタイプ（ピクセルフォーマット）を取得
            if let Ok(subtype) = media_type.GetGUID(&MF_MT_SUBTYPE)
                && let Some(pixel_format) = guid_to_pixel_format(&subtype)
                && let Ok(frame_size) = media_type.GetUINT64(&MF_MT_FRAME_SIZE)
            {
                let width = (frame_size >> 32) as i32;
                let height = (frame_size & 0xFFFFFFFF) as i32;

                // フレームレートを取得
                let (min_fps, max_fps) =
                    if let Ok(frame_rate) = media_type.GetUINT64(&MF_MT_FRAME_RATE) {
                        let numerator = (frame_rate >> 32) as f32;
                        let denominator = (frame_rate & 0xFFFFFFFF) as f32;
                        let fps = if denominator > 0.0 {
                            numerator / denominator
                        } else {
                            30.0
                        };
                        (fps, fps)
                    } else {
                        (1.0, 30.0)
                    };

                formats.push(VideoFormat {
                    width,
                    height,
                    min_fps,
                    max_fps,
                    pixel_format,
                });
            }

            index += 1;
        }

        // ソースリーダーを先にスコープ外へ（明示的に解放）してからソースを Shutdown する（読み手が順序を追いやすくする）
        drop(reader);
        let _ = source.Shutdown();
    }

    formats
}

/// デバイスを列挙
fn enumerate_devices_internal() -> Result<Vec<VideoDevice>> {
    unsafe {
        let _com_guard = CoInitGuard::new()?;

        // MFStartup が失敗した場合は参照カウントを増やしていないため MFShutdown は呼ばない（MSDN の初期化契約に従う）。
        MFStartup(MF_VERSION, MFSTARTUP_NOSOCKET).map_err(|_| Error::DeviceAccessDenied)?;

        let result = enumerate_devices_impl();

        // この関数内で成功した MFStartup と対になる MFShutdown
        let _ = MFShutdown();

        result
    }
}

/// デバイス列挙の内部実装
unsafe fn enumerate_devices_impl() -> Result<Vec<VideoDevice>> {
    unsafe {
        // デバイス列挙用の属性を作成
        let mut attributes: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attributes, 1).map_err(|_| Error::DeviceAccessDenied)?;
        let attributes = attributes.ok_or(Error::DeviceAccessDenied)?;

        attributes
            .SetGUID(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            )
            .map_err(|_| Error::DeviceAccessDenied)?;

        // デバイスを列挙
        let mut devices_ptr: *mut Option<IMFActivate> = ptr::null_mut();
        let mut count: u32 = 0;

        MFEnumDeviceSources(&attributes, &mut devices_ptr, &mut count)
            .map_err(|_| Error::DeviceAccessDenied)?;

        let mut devices = Vec::new();

        if count > 0 && !devices_ptr.is_null() {
            let _devices_guard = CoTaskMemActivateArrayGuard { ptr: devices_ptr };

            let device_slice = std::slice::from_raw_parts(devices_ptr, count as usize);

            for activate in device_slice.iter().flatten() {
                // デバイス名を取得
                let name = get_device_string(activate, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)
                    .unwrap_or_else(|| "Unknown Device".to_string());

                // デバイス ID を取得
                let unique_id = get_device_string(
                    activate,
                    &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
                )
                .unwrap_or_else(|| format!("device_{}", devices.len()));

                // フォーマット情報を取得
                let formats = get_device_formats(activate);

                devices.push(VideoDevice {
                    name,
                    unique_id,
                    formats,
                });
            }
        }

        Ok(devices)
    }
}

/// デバイスから文字列属性を取得
fn get_device_string(activate: &IMFActivate, key: &GUID) -> Option<String> {
    unsafe {
        let length = activate.GetStringLength(key).ok()?;
        if length == 0 {
            return None;
        }

        let mut buffer: Vec<u16> = vec![0; (length + 1) as usize];
        activate.GetString(key, &mut buffer, None).ok()?;

        String::from_utf16(&buffer[..length as usize]).ok()
    }
}
