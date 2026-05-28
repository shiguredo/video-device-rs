//! ビデオデバイスとデバイスリストの定義。
//!
//! バックエンドに依存しない形でデバイス情報（名前、一意識別子、対応フォーマット）を
//! 取得する [`VideoDevice`] と、デバイス列挙の [`VideoDeviceList`] を提供する。

use crate::error::Result;
use crate::types::VideoFormat;

#[cfg(any(target_os = "macos", target_os = "linux"))]
use crate::device_ffi::{FfiDeviceImpl, FfiDeviceListImpl};

#[cfg(target_os = "windows")]
use crate::device_mf::{MfDeviceImpl, MfDeviceListImpl};

/// ビデオデバイス。
///
/// バックエンドに依存しないラッパーで、各プラットフォームに応じた
/// FFI 実装または Windows ネイティブ実装を内包する。
/// `Send + Sync` であるため、スレッド間共有が可能。
pub struct VideoDevice<'a>(pub(crate) VideoDeviceInner<'a>);

pub(crate) enum VideoDeviceInner<'a> {
    /// macOS / Linux の FFI ベースデバイス。
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Ffi(FfiDeviceImpl<'a>),
    /// Windows Media Foundation ベースデバイス。
    #[cfg(target_os = "windows")]
    Mf(MfDeviceImpl),
}

impl VideoDevice<'_> {
    /// デバイス名を取得する。
    ///
    /// FFI バックエンドでは C 側がヌルポインタを返しうるため `Result` を返す。
    pub fn name(&self) -> Result<String> {
        match &self.0 {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            VideoDeviceInner::Ffi(inner) => inner.name(),
            #[cfg(target_os = "windows")]
            VideoDeviceInner::Mf(inner) => inner.name(),
        }
    }

    /// デバイスの一意識別子を取得する。
    ///
    /// FFI バックエンドでは C 側がヌルポインタを返しうるため `Result` を返す。
    pub fn unique_id(&self) -> Result<String> {
        match &self.0 {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            VideoDeviceInner::Ffi(inner) => inner.unique_id(),
            #[cfg(target_os = "windows")]
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
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            VideoDeviceInner::Ffi(inner) => inner.format_count(),
            #[cfg(target_os = "windows")]
            VideoDeviceInner::Mf(inner) => inner.format_count(),
        }
    }

    /// 対応フォーマット一覧を取得する。
    ///
    /// 実際に取得できたフォーマットのリストである。
    /// [`format_count`](VideoDevice::format_count) の値と一致しない場合がある（上記のスキップのため）。
    pub fn formats(&self) -> Vec<VideoFormat> {
        match &self.0 {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            VideoDeviceInner::Ffi(inner) => inner.formats(),
            #[cfg(target_os = "windows")]
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
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Ffi {
        _inner: FfiDeviceListImpl,
        devices: Vec<VideoDevice<'static>>,
    },
    /// Windows Media Foundation ベースデバイスリスト。
    #[cfg(target_os = "windows")]
    Mf { devices: Vec<VideoDevice<'static>> },
}

impl VideoDeviceList {
    /// macOS AVFoundation でデバイスを列挙する。
    #[cfg(target_os = "macos")]
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
    #[cfg(all(target_os = "linux", feature = "v4l2"))]
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
    #[cfg(all(target_os = "linux", feature = "pipewire"))]
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
    #[cfg(target_os = "windows")]
    pub fn enumerate_mf() -> Result<Self> {
        let list = MfDeviceListImpl::enumerate()?;
        let devices = list
            .devices
            .into_iter()
            .map(|d| VideoDevice(VideoDeviceInner::Mf(d)))
            .collect();
        Ok(Self(VideoDeviceListInner::Mf { devices }))
    }

    /// デバイスのスライスを取得する。
    ///
    /// 戻り値のスライスが参照するメモリは `self` のライフタイムに束縛される。
    pub fn devices(&self) -> &[VideoDevice<'static>] {
        match &self.0 {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            VideoDeviceListInner::Ffi { devices, .. } => devices,
            #[cfg(target_os = "windows")]
            VideoDeviceListInner::Mf { devices } => devices,
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
    type Item = &'a VideoDevice<'static>;
    type IntoIter = std::slice::Iter<'a, VideoDevice<'static>>;

    fn into_iter(self) -> Self::IntoIter {
        self.devices().iter()
    }
}
