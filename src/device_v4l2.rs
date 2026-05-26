use std::ffi::CStr;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::error::{Error, Result};
use crate::ffi;
use crate::types::{PixelFormat, VideoFormat};
use crate::{VideoDevice, VideoDeviceList};

/// V4L2 ビデオデバイス
pub struct V4l2VideoDevice<'a> {
    raw: NonNull<ffi::v4l2::VideoDevice>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> VideoDevice for V4l2VideoDevice<'a> {
    fn name(&self) -> Result<String> {
        let name_ptr = unsafe { ffi::v4l2::video_v4l2_device_name(self.raw.as_ptr()) };
        if name_ptr.is_null() {
            return Err(Error::NullPointer("device name"));
        }
        let name = unsafe { CStr::from_ptr(name_ptr) };
        Ok(name.to_string_lossy().into_owned())
    }

    fn unique_id(&self) -> Result<String> {
        let id_ptr = unsafe { ffi::v4l2::video_v4l2_device_unique_id(self.raw.as_ptr()) };
        if id_ptr.is_null() {
            return Err(Error::NullPointer("device unique_id"));
        }
        let id = unsafe { CStr::from_ptr(id_ptr) };
        Ok(id.to_string_lossy().into_owned())
    }

    fn format_count(&self) -> usize {
        let count = unsafe { ffi::v4l2::video_v4l2_device_format_count(self.raw.as_ptr()) };
        count.max(0) as usize
    }

    fn formats(&self) -> Vec<VideoFormat> {
        let count = self.format_count();
        let mut formats = Vec::with_capacity(count);

        for i in 0..count {
            let format_ptr =
                unsafe { ffi::v4l2::video_v4l2_device_get_format(self.raw.as_ptr(), i as i32) };
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

// SAFETY: FFI 側の VideoDevice はデバイスリストが所有する read-only データであり、
// 複数スレッドから同時に読むことは安全。
unsafe impl Send for V4l2VideoDevice<'_> {}
unsafe impl Sync for V4l2VideoDevice<'_> {}

/// V4L2 ビデオデバイスリスト
pub struct V4l2VideoDeviceList {
    devices_ptr: *mut *mut ffi::v4l2::VideoDevice,
    // C が返した全エントリ数。devices.len() とは異なる場合がある（NULL エントリ除外のため）。
    // Drop で video_v4l2_free_devices に渡す際はこの値を使う（C 側の確保サイズに対応）。
    count: i32,
    devices: Vec<V4l2VideoDevice<'static>>,
}

impl VideoDeviceList for V4l2VideoDeviceList {
    type Device<'a> = V4l2VideoDevice<'a>;

    fn devices(&self) -> &[Self::Device<'_>] {
        &self.devices
    }
}

impl V4l2VideoDeviceList {
    /// デバイスを列挙
    pub fn enumerate() -> Result<Self> {
        let mut devices_ptr: *mut *mut ffi::v4l2::VideoDevice = std::ptr::null_mut();
        let mut count: i32 = 0;

        let ret = unsafe { ffi::v4l2::video_v4l2_enumerate_devices(&mut devices_ptr, &mut count) };
        if ret < 0 {
            return Err(Error::DeviceAccessDenied);
        }

        let devices: Vec<V4l2VideoDevice<'static>> = if count > 0 && !devices_ptr.is_null() {
            (0..count as usize)
                .filter_map(|i| {
                    let device_ptr = unsafe { *devices_ptr.add(i) };
                    NonNull::new(device_ptr).map(|raw| V4l2VideoDevice {
                        raw,
                        _phantom: PhantomData,
                    })
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
}

impl Drop for V4l2VideoDeviceList {
    fn drop(&mut self) {
        if !self.devices_ptr.is_null() {
            unsafe {
                ffi::v4l2::video_v4l2_free_devices(self.devices_ptr, self.count);
            }
        }
    }
}

// SAFETY: FFI 側のデバイスリストは read-only であり、スレッド間共有は安全。
unsafe impl Send for V4l2VideoDeviceList {}
unsafe impl Sync for V4l2VideoDeviceList {}
