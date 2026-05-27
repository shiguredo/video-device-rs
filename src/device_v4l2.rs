//! Linux V4L2 用のビデオデバイス列挙。

use crate::error::Result;
use crate::ffi;
use crate::types::VideoFormat;
use crate::device_common::{DeviceInner, DeviceListInner, DeviceOps};

/// V4L2 の FFI 関数テーブル。
const OPS: DeviceOps = DeviceOps {
    device_name:        ffi::video_v4l2_device_name,
    device_unique_id:   ffi::video_v4l2_device_unique_id,
    device_format_count: ffi::video_v4l2_device_format_count,
    device_get_format:  ffi::video_v4l2_device_get_format,
    enumerate_devices:  ffi::video_v4l2_enumerate_devices,
    free_devices:       ffi::video_v4l2_free_devices,
};

/// Linux V4L2 ビデオデバイス。
pub struct V4l2VideoDevice<'a> {
    inner: DeviceInner<'a>,
}

impl<'a> crate::VideoDevice for V4l2VideoDevice<'a> {
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

/// Linux V4L2 ビデオデバイスリスト。
pub struct V4l2VideoDeviceList {
    _inner: DeviceListInner,
    devices: Vec<V4l2VideoDevice<'static>>,
}

impl V4l2VideoDeviceList {
    pub fn enumerate() -> Result<Self> {
        let inner = DeviceListInner::enumerate(&OPS)?;
        let devices = inner
            .devices()
            .iter()
            .map(|d| V4l2VideoDevice { inner: *d })
            .collect();
        Ok(Self { _inner: inner, devices })
    }
}

impl crate::VideoDeviceList for V4l2VideoDeviceList {
    type Device<'a> = V4l2VideoDevice<'a>;

    fn devices(&self) -> &[Self::Device<'_>] {
        &self.devices
    }
}
