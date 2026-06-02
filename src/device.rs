//! ビデオデバイスとデバイスリストの定義。
//!
//! バックエンドに依存しない形でデバイス情報（名前、一意識別子、対応フォーマット）を
//! 取得する [`VideoDevice`] と、デバイス列挙の [`VideoDeviceList`] を提供する。

use crate::error::Result;
use crate::types::VideoFormat;

#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
use crate::device_ffi::{FfiDeviceImpl, FfiDeviceListImpl};

#[cfg(enable_mf)]
use crate::device_mf::{MfDeviceImpl, MfDeviceListImpl};

/// ビデオデバイス。
///
/// バックエンドに依存しないラッパーで、各プラットフォームに応じた
/// FFI 実装または Windows ネイティブ実装を内包する。
/// `Send + Sync` であるため、スレッド間共有が可能。
pub struct VideoDevice(pub(crate) VideoDeviceInner);

pub(crate) enum VideoDeviceInner {
    /// macOS / Linux の FFI ベースデバイス。
    #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
    Ffi(FfiDeviceImpl),
    /// Windows Media Foundation ベースデバイス。
    #[cfg(enable_mf)]
    Mf(MfDeviceImpl),
}

impl VideoDevice {
    /// デバイス名を取得する。
    ///
    /// FFI バックエンドでは C 側がヌルポインタを返しうるため `Result` を返す。
    pub fn name(&self) -> Result<String> {
        match &self.0 {
            #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
            VideoDeviceInner::Ffi(inner) => inner.name(),
            #[cfg(enable_mf)]
            VideoDeviceInner::Mf(inner) => inner.name(),
        }
    }

    /// デバイスの一意識別子を取得する。
    ///
    /// FFI バックエンドでは C 側がヌルポインタを返しうるため `Result` を返す。
    pub fn unique_id(&self) -> Result<String> {
        match &self.0 {
            #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
            VideoDeviceInner::Ffi(inner) => inner.unique_id(),
            #[cfg(enable_mf)]
            VideoDeviceInner::Mf(inner) => inner.unique_id(),
        }
    }

    /// 対応フォーマット数を取得する。
    ///
    /// C 側が報告するエントリ数（インデックスの上限）である。
    /// [`formats`](VideoDevice::formats) は `NULL` で取得できなかったインデックスをスキップするため、
    /// 返すベクタの要素数がこれより少ない場合がある。
    pub fn format_count(&self) -> usize {
        match &self.0 {
            #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
            VideoDeviceInner::Ffi(inner) => inner.format_count(),
            #[cfg(enable_mf)]
            VideoDeviceInner::Mf(inner) => inner.format_count(),
        }
    }

    /// 対応フォーマット一覧を取得する。
    ///
    /// 実際に取得できたフォーマットのリストである。
    /// [`format_count`](VideoDevice::format_count) の値と一致しない場合がある（上記のスキップのため）。
    pub fn formats(&self) -> Vec<VideoFormat> {
        match &self.0 {
            #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
            VideoDeviceInner::Ffi(inner) => inner.formats(),
            #[cfg(enable_mf)]
            VideoDeviceInner::Mf(inner) => inner.formats(),
        }
    }
}

/// ビデオデバイスリスト。
///
/// 各プラットフォームのデバイス列挙結果を保持する。
/// `Send + Sync` であるため、スレッド間共有が可能。
pub struct VideoDeviceList(pub(crate) VideoDeviceListInner);

pub(crate) enum VideoDeviceListInner {
    /// macOS / Linux の FFI ベースデバイスリスト。
    #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
    Ffi {
        _inner: FfiDeviceListImpl,
        devices: Vec<VideoDevice>,
    },
    /// Windows Media Foundation ベースデバイスリスト。
    #[cfg(enable_mf)]
    Mf {
        _inner: MfDeviceListImpl,
        devices: Vec<VideoDevice>,
    },
}

impl VideoDeviceList {
    /// macOS AVFoundation でデバイスを列挙する。
    #[cfg(enable_avf)]
    pub fn enumerate_avf() -> Result<Self> {
        let inner = FfiDeviceListImpl::enumerate_avf()?;
        let devices = inner
            .devices()
            .iter()
            .map(|d| VideoDevice(VideoDeviceInner::Ffi(*d)))
            .collect();
        Ok(Self(VideoDeviceListInner::Ffi {
            _inner: inner,
            devices,
        }))
    }

    /// Linux V4L2 でデバイスを列挙する。
    #[cfg(enable_v4l2)]
    pub fn enumerate_v4l2() -> Result<Self> {
        let inner = FfiDeviceListImpl::enumerate_v4l2()?;
        let devices = inner
            .devices()
            .iter()
            .map(|d| VideoDevice(VideoDeviceInner::Ffi(*d)))
            .collect();
        Ok(Self(VideoDeviceListInner::Ffi {
            _inner: inner,
            devices,
        }))
    }

    /// Linux PipeWire でデバイスを列挙する。
    #[cfg(enable_pipewire)]
    pub fn enumerate_pipewire() -> Result<Self> {
        let inner = FfiDeviceListImpl::enumerate_pipewire()?;
        let devices = inner
            .devices()
            .iter()
            .map(|d| VideoDevice(VideoDeviceInner::Ffi(*d)))
            .collect();
        Ok(Self(VideoDeviceListInner::Ffi {
            _inner: inner,
            devices,
        }))
    }

    /// Windows Media Foundation でデバイスを列挙する。
    #[cfg(enable_mf)]
    pub fn enumerate_mf() -> Result<Self> {
        let mut inner = MfDeviceListImpl::enumerate()?;
        let raw_devices = std::mem::take(&mut inner.devices);
        let devices = raw_devices
            .into_iter()
            .map(|d| VideoDevice(VideoDeviceInner::Mf(d)))
            .collect();
        Ok(Self(VideoDeviceListInner::Mf {
            _inner: inner,
            devices,
        }))
    }

    /// デバイスのスライスを取得する。
    ///
    /// 戻り値のスライスが参照するメモリは `self` のライフタイムに束縛される。
    pub fn devices(&self) -> &[VideoDevice] {
        match &self.0 {
            #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
            VideoDeviceListInner::Ffi { devices, .. } => devices,
            #[cfg(enable_mf)]
            VideoDeviceListInner::Mf { devices, .. } => devices,
        }
    }

    /// デバイス数を取得する。
    pub fn len(&self) -> usize {
        self.devices().len()
    }

    /// デバイスが空かどうかを返す。
    pub fn is_empty(&self) -> bool {
        self.devices().is_empty()
    }
}

impl<'a> IntoIterator for &'a VideoDeviceList {
    type Item = &'a VideoDevice;
    type IntoIter = std::slice::Iter<'a, VideoDevice>;

    fn into_iter(self) -> Self::IntoIter {
        self.devices().iter()
    }
}
