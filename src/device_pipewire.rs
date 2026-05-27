//! Linux PipeWire 用のビデオデバイス列挙。

use crate::error::Result;
use crate::ffi;
use crate::types::VideoFormat;
use crate::device_common::{DeviceInner, DeviceListInner, DeviceOps};

/// PipeWire の FFI 関数テーブル。
const OPS: DeviceOps = DeviceOps {
    device_name:        ffi::video_pipewire_device_name,
    device_unique_id:   ffi::video_pipewire_device_unique_id,
    device_format_count: ffi::video_pipewire_device_format_count,
    device_get_format:  ffi::video_pipewire_device_get_format,
    enumerate_devices:  ffi::video_pipewire_enumerate_devices,
    free_devices:       ffi::video_pipewire_free_devices,
};

/// Linux PipeWire ビデオデバイス。
pub struct PipewireVideoDevice<'a> {
    inner: DeviceInner<'a>,
}

impl<'a> crate::VideoDevice for PipewireVideoDevice<'a> {
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

/// Linux PipeWire ビデオデバイスリスト。
pub struct PipewireVideoDeviceList {
    _inner: DeviceListInner,
    devices: Vec<PipewireVideoDevice<'static>>,
}

impl PipewireVideoDeviceList {
    pub fn enumerate() -> Result<Self> {
        let inner = DeviceListInner::enumerate(&OPS)?;
        let devices = inner
            .devices()
            .iter()
            .map(|d| PipewireVideoDevice { inner: *d })
            .collect();
        Ok(Self { _inner: inner, devices })
    }
}

impl crate::VideoDeviceList for PipewireVideoDeviceList {
    type Device<'a> = PipewireVideoDevice<'a>;

    fn devices(&self) -> &[Self::Device<'_>] {
        &self.devices
    }
}
