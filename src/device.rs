use std::ffi::CStr;
use std::ptr::NonNull;

use crate::error::{Error, Result};
use crate::ffi;
use crate::types::{PixelFormat, VideoFormat};

/// ビデオデバイス
pub struct VideoDevice {
    raw: NonNull<ffi::VideoDevice>,
}

impl VideoDevice {
    /// デバイス名を取得
    pub fn name(&self) -> Result<String> {
        let name_ptr = unsafe { ffi::video_device_name(self.raw.as_ptr()) };
        if name_ptr.is_null() {
            return Err(Error::NullPointer("device name"));
        }
        let name = unsafe { CStr::from_ptr(name_ptr) };
        Ok(name.to_string_lossy().into_owned())
    }

    /// デバイスの一意識別子を取得
    pub fn unique_id(&self) -> Result<String> {
        let id_ptr = unsafe { ffi::video_device_unique_id(self.raw.as_ptr()) };
        if id_ptr.is_null() {
            return Err(Error::NullPointer("device unique_id"));
        }
        let id = unsafe { CStr::from_ptr(id_ptr) };
        Ok(id.to_string_lossy().into_owned())
    }

    /// 対応フォーマット数を取得
    ///
    /// C 側が報告するエントリ数（インデックスの上限）である。
    /// [`Self::formats`] は `NULL` で取得できなかったインデックスをスキップするため、
    /// 返すベクタの要素数がこれより少ない場合がある。
    pub fn format_count(&self) -> usize {
        let count = unsafe { ffi::video_device_format_count(self.raw.as_ptr()) };
        count.max(0) as usize
    }

    /// 対応フォーマット一覧を取得
    ///
    /// 実際に取得できたフォーマットのリストである。
    /// [`Self::format_count`](Self::format_count) の値と一致しない場合がある（上記のスキップのため）。
    pub fn formats(&self) -> Vec<VideoFormat> {
        let count = self.format_count();
        let mut formats = Vec::with_capacity(count);

        for i in 0..count {
            let format_ptr = unsafe { ffi::video_device_get_format(self.raw.as_ptr(), i as i32) };
            if format_ptr.is_null() {
                continue;
            }

            let format = unsafe { &*format_ptr };
            formats.push(VideoFormat {
                width: format.width,
                height: format.height,
                min_fps: format.min_fps,
                max_fps: format.max_fps,
                pixel_format: PixelFormat::from_raw(format.pixel_format),
            });
        }

        formats
    }
}

// VideoDevice は FFI ポインタを持つが、内部データはスレッドセーフ
unsafe impl Send for VideoDevice {}
unsafe impl Sync for VideoDevice {}

/// ビデオデバイスリスト
pub struct VideoDeviceList {
    devices_ptr: *mut *mut ffi::VideoDevice,
    count: i32,
    devices: Vec<VideoDevice>,
}

impl VideoDeviceList {
    /// デバイスを列挙
    pub fn enumerate() -> Result<Self> {
        let mut devices_ptr: *mut *mut ffi::VideoDevice = std::ptr::null_mut();
        let mut count: i32 = 0;

        let ret = unsafe { ffi::video_enumerate_devices(&mut devices_ptr, &mut count) };
        if ret < 0 {
            return Err(Error::DeviceAccessDenied);
        }

        let devices: Vec<VideoDevice> = if count > 0 && !devices_ptr.is_null() {
            (0..count as usize)
                .filter_map(|i| {
                    let device_ptr = unsafe { *devices_ptr.add(i) };
                    NonNull::new(device_ptr).map(|raw| VideoDevice { raw })
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok(Self {
            devices_ptr,
            count,
            devices,
        })
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

impl Drop for VideoDeviceList {
    fn drop(&mut self) {
        if !self.devices_ptr.is_null() {
            unsafe {
                ffi::video_free_devices(self.devices_ptr, self.count);
            }
        }
    }
}

impl<'a> IntoIterator for &'a VideoDeviceList {
    type Item = &'a VideoDevice;
    type IntoIter = std::slice::Iter<'a, VideoDevice>;

    fn into_iter(self) -> Self::IntoIter {
        self.devices.iter()
    }
}

// VideoDeviceList は FFI ポインタを持つが、内部データはスレッドセーフ
unsafe impl Send for VideoDeviceList {}
unsafe impl Sync for VideoDeviceList {}
