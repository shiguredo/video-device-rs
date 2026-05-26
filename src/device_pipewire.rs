use std::ffi::CStr;
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::error::{Error, Result};
use crate::ffi;
use crate::types::VideoFormat;
use crate::{VideoDevice, VideoDeviceList};

/// PipeWire ビデオデバイス
pub struct PipewireVideoDevice<'a> {
    raw: NonNull<ffi::pipewire::VideoDevice>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> VideoDevice for PipewireVideoDevice<'a> {
    fn name(&self) -> Result<String> {
        let name_ptr = unsafe { ffi::pipewire::video_pipewire_device_name(self.raw.as_ptr()) };
        if name_ptr.is_null() {
            return Err(Error::NullPointer("device name"));
        }
        let name = unsafe { CStr::from_ptr(name_ptr) };
        Ok(name.to_string_lossy().into_owned())
    }

    fn unique_id(&self) -> Result<String> {
        let id_ptr = unsafe { ffi::pipewire::video_pipewire_device_unique_id(self.raw.as_ptr()) };
        if id_ptr.is_null() {
            return Err(Error::NullPointer("device unique_id"));
        }
        let id = unsafe { CStr::from_ptr(id_ptr) };
        Ok(id.to_string_lossy().into_owned())
    }

    fn format_count(&self) -> usize {
        let count = unsafe { ffi::pipewire::video_pipewire_device_format_count(self.raw.as_ptr()) };
        count.max(0) as usize
    }

    fn formats(&self) -> Vec<VideoFormat> {
        let count = self.format_count();
        let mut formats = Vec::with_capacity(count);

        for i in 0..count {
            let format_ptr = unsafe {
                ffi::pipewire::video_pipewire_device_get_format(self.raw.as_ptr(), i as i32)
            };
            if format_ptr.is_null() {
                continue;
            }

            let format = unsafe { &*format_ptr };
            formats.push(VideoFormat {
                width: format.width,
                height: format.height,
                min_fps: format.min_fps,
                max_fps: format.max_fps,
                pixel_format: crate::types::PixelFormat::from_raw(format.pixel_format),
            });
        }

        formats
    }
}

// SAFETY: FFI 側の VideoDevice はデバイスリストが所有する read-only データであり、
// 複数スレッドから同時に読むことは安全。
unsafe impl Send for PipewireVideoDevice<'_> {}
unsafe impl Sync for PipewireVideoDevice<'_> {}

/// PipeWire ビデオデバイスリスト
pub struct PipewireVideoDeviceList {
    devices_ptr: *mut *mut ffi::pipewire::VideoDevice,
    count: i32,
    devices: Vec<PipewireVideoDevice<'static>>,
}

impl VideoDeviceList for PipewireVideoDeviceList {
    type Device<'a> = PipewireVideoDevice<'a>;

    fn devices(&self) -> &[Self::Device<'_>] {
        &self.devices
    }
}

impl PipewireVideoDeviceList {
    /// デバイスを列挙
    pub fn enumerate() -> Result<Self> {
        let mut devices_ptr: *mut *mut ffi::pipewire::VideoDevice = std::ptr::null_mut();
        let mut count: i32 = 0;

        let ret = unsafe {
            ffi::pipewire::video_pipewire_enumerate_devices(&mut devices_ptr, &mut count)
        };
        if ret < 0 {
            return Err(Error::DeviceAccessDenied);
        }

        let devices: Vec<PipewireVideoDevice<'static>> = if count > 0 && !devices_ptr.is_null() {
            (0..count as usize)
                .filter_map(|i| {
                    let device_ptr = unsafe { *devices_ptr.add(i) };
                    NonNull::new(device_ptr).map(|raw| PipewireVideoDevice {
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

impl Drop for PipewireVideoDeviceList {
    fn drop(&mut self) {
        if !self.devices_ptr.is_null() {
            unsafe {
                ffi::pipewire::video_pipewire_free_devices(self.devices_ptr, self.count);
            }
        }
    }
}

// SAFETY: FFI 側のデバイスリストは read-only であり、スレッド間共有は安全。
unsafe impl Send for PipewireVideoDeviceList {}
unsafe impl Sync for PipewireVideoDeviceList {}
