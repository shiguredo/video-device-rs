//! macOS AVFoundation 用のビデオデバイス列挙。

use crate::device_ffi::{DeviceInner, DeviceListInner, DeviceOps};
use crate::error::Result;
use crate::ffi;
use crate::types::VideoFormat;

/// AVFoundation の FFI 関数テーブル。
const OPS: DeviceOps = DeviceOps {
    device_name: ffi::video_avf_device_name,
    device_unique_id: ffi::video_avf_device_unique_id,
    device_format_count: ffi::video_avf_device_format_count,
    device_get_format: ffi::video_avf_device_get_format,
    enumerate_devices: ffi::video_avf_enumerate_devices,
    free_devices: ffi::video_avf_free_devices,
};

/// macOS AVFoundation ビデオデバイス。
pub struct AvfVideoDevice<'a> {
    inner: DeviceInner<'a>,
}

impl<'a> crate::VideoDevice for AvfVideoDevice<'a> {
    fn name(&self) -> Result<String> {
        self.inner.name()
    }

    fn unique_id(&self) -> Result<String> {
        self.inner.unique_id()
    }

    fn format_count(&self) -> usize {
        self.inner.format_count()
    }

    fn formats(&self) -> Vec<VideoFormat> {
        self.inner.formats()
    }
}

/// macOS AVFoundation ビデオデバイスリスト。
pub struct AvfVideoDeviceList {
    _inner: DeviceListInner,
    devices: Vec<AvfVideoDevice<'static>>,
}

impl AvfVideoDeviceList {
    pub fn enumerate() -> Result<Self> {
        let inner = DeviceListInner::enumerate(&OPS)?;
        let devices = inner
            .devices()
            .iter()
            .map(|d| AvfVideoDevice { inner: *d })
            .collect();
        Ok(Self {
            _inner: inner,
            devices,
        })
    }
}

impl crate::VideoDeviceList for AvfVideoDeviceList {
    type Device<'a> = AvfVideoDevice<'a>;

    fn devices(&self) -> &[Self::Device<'_>] {
        &self.devices
    }
}
