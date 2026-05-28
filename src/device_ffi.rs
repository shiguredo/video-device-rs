//! macOS / Linux 共通のビデオデバイス実装。
//!
//! バックエンドごとに FFI 関数群を [`DeviceOps`] で渡し、[`FfiDeviceImpl`] と
//! [`FfiDeviceListImpl`] がデバイス情報取得 / デバイス列挙を一括で提供する。

use std::ffi::{CStr, c_char};
use std::marker::PhantomData;
use std::ptr::NonNull;

use crate::error::{Error, Result};
use crate::ffi;
use crate::types::{PixelFormat, VideoFormat};

// ---------------------------------------------------------------------------
// バックエンドとの境界
// ---------------------------------------------------------------------------

/// バックエンド固有の FFI 関数テーブル。
///
/// 各プラットフォームの FFI OPS は本ファイル末尾で `const` として定義し、
/// `FfiDeviceImpl` / `FfiDeviceListImpl` に `&'static` で渡す。
struct DeviceOps {
    /// デバイス名を示す NUL 終端 UTF-8 文字列へのポインタを返す。
    pub device_name: unsafe extern "C" fn(device: *mut ffi::VideoDevice) -> *const c_char,
    /// デバイスの一意識別子を示す NUL 終端 UTF-8 文字列へのポインタを返す。
    pub device_unique_id: unsafe extern "C" fn(device: *mut ffi::VideoDevice) -> *const c_char,
    /// デバイスが報告するフォーマットエントリ数を返す。
    pub device_format_count: unsafe extern "C" fn(device: *mut ffi::VideoDevice) -> i32,
    /// 指定インデックスのフォーマットエントリへのポインタを返す。
    pub device_get_format: unsafe extern "C" fn(
        device: *mut ffi::VideoDevice,
        index: i32,
    ) -> *const ffi::VideoFormatEntry,
    /// システム上の全デバイスを列挙し、ポインタ配列と要素数を出力パラメータで返す。
    pub enumerate_devices:
        unsafe extern "C" fn(devices_ptr: *mut *mut *mut ffi::VideoDevice, count: *mut i32) -> i32,
    /// `enumerate_devices` で確保されたポインタ配列を解放する。
    pub free_devices: unsafe extern "C" fn(devices_ptr: *mut *mut ffi::VideoDevice, count: i32),
}

// ---------------------------------------------------------------------------
// 単一デバイス
// ---------------------------------------------------------------------------

/// バックエンド非依存の単一デバイス実装。
///
/// `FfiDeviceListImpl` が C のポインタ配列から生成し、`Clone + Copy` であるため、
/// デバイスリスト構築時の複製は軽量。
#[derive(Clone, Copy)]
pub(crate) struct FfiDeviceImpl<'a> {
    /// バックエンドの FFI 関数テーブル。
    ops: &'static DeviceOps,
    /// C の `VideoDevice` へのポインタ。
    raw: NonNull<ffi::VideoDevice>,
    /// 本デバイスのライフタイムをデバイスリストの借用に束縛するためのマーカー。
    _phantom: PhantomData<&'a ()>,
}

impl FfiDeviceImpl<'_> {
    pub fn name(&self) -> Result<String> {
        // SAFETY: raw は enumerate_devices が返した有効なポインタであり、
        // FfiDeviceListImpl のライフタイム中は C 側のメモリが有効。
        let name_ptr = unsafe { (self.ops.device_name)(self.raw.as_ptr()) };
        if name_ptr.is_null() {
            return Err(Error::NullPointer("device name"));
        }
        // SAFETY: C 側が返す文字列は NUL 終端であり、デバイスリストのライフタイム中有効。
        let name = unsafe { CStr::from_ptr(name_ptr) }
            .to_str()
            .map_err(|_| Error::InvalidUtf8("device name"))?
            .to_owned();
        Ok(name)
    }

    pub fn unique_id(&self) -> Result<String> {
        // SAFETY: name() と同様の安全性保証。
        let id_ptr = unsafe { (self.ops.device_unique_id)(self.raw.as_ptr()) };
        if id_ptr.is_null() {
            return Err(Error::NullPointer("device unique_id"));
        }
        let id = unsafe { CStr::from_ptr(id_ptr) }
            .to_str()
            .map_err(|_| Error::InvalidUtf8("device unique_id"))?
            .to_owned();
        Ok(id)
    }

    pub fn format_count(&self) -> usize {
        let count = unsafe { (self.ops.device_format_count)(self.raw.as_ptr()) };
        count.max(0) as usize
    }

    pub fn formats(&self) -> Vec<VideoFormat> {
        let count = self.format_count();
        if count == 0 {
            return Vec::new();
        }

        let mut formats = Vec::with_capacity(count);
        for i in 0..count {
            // SAFETY: i は [0, count) の範囲であり、C 側が有効なインデックスに対して
            // フォーマット情報を返すことを保証している。
            let format_ptr = unsafe { (self.ops.device_get_format)(self.raw.as_ptr(), i as i32) };
            if format_ptr.is_null() {
                continue;
            }
            // SAFETY: format_ptr が非 NULL の場合、C 側の有効な VideoFormatEntry を指している。
            let f = unsafe { &*format_ptr };
            formats.push(VideoFormat {
                width: f.width,
                height: f.height,
                min_fps: f.min_fps,
                max_fps: f.max_fps,
                pixel_format: PixelFormat::from_raw(f.pixel_format),
            });
        }
        formats
    }
}

// SAFETY: 内部に保持する FFI デバイスポインタは C 側のスレッド安全性に従う。
// 全バックエンドでデバイス情報は read-only であり、複数スレッドからの参照は安全。
unsafe impl Send for FfiDeviceImpl<'_> {}
unsafe impl Sync for FfiDeviceImpl<'_> {}

// ---------------------------------------------------------------------------
// デバイスリスト（列挙・解放を内包）
// ---------------------------------------------------------------------------

/// バックエンド非依存のデバイスリスト実装。
///
/// C 側で列挙されたポインタ配列の所有権と、そこから構築された
/// `FfiDeviceImpl` ベクタの両方を保持する。`Drop` で C 側の配列を解放する。
pub(crate) struct FfiDeviceListImpl {
    /// バックエンドの FFI 関数テーブル。
    ops: &'static DeviceOps,
    /// `enumerate_devices` が確保したポインタ配列（`Drop` で解放）。
    devices_ptr: *mut *mut ffi::VideoDevice,
    /// C が返した全エントリ数。
    ///
    /// `devices.len()` とは異なる場合がある（NULL エントリ除外のため）。
    /// `Drop` で `free_devices` に渡す際はこの値を使う（C 側の確保サイズに対応）。
    count: i32,
    /// 有効なデバイスの一覧（NULL エントリを除外済み）。
    devices: Vec<FfiDeviceImpl<'static>>,
}

impl FfiDeviceListImpl {
    /// システム上の全デバイスを列挙する。
    fn enumerate(ops: &'static DeviceOps) -> Result<Self> {
        let mut devices_ptr: *mut *mut ffi::VideoDevice = std::ptr::null_mut();
        let mut count: i32 = 0;

        // SAFETY: C 側が出力パラメータに有効なポインタを書き込むことを期待する。
        // devices_ptr が NULL でない場合、呼び出し側が free_devices で解放する責任を負う。
        let ret = unsafe { (ops.enumerate_devices)(&mut devices_ptr, &mut count) };
        if ret < 0 || devices_ptr.is_null() {
            return Err(Error::DeviceAccessDenied);
        }

        let devices: Vec<FfiDeviceImpl<'static>> = (0..count as usize)
            .filter_map(|i| {
                // SAFETY: i は [0, count) の範囲であり、C 側が有効な配列を保証する。
                // NULL エントリはフィルタで除外する（一部のバックエンドで発生しうる）。
                let device_ptr = unsafe { *devices_ptr.add(i) };
                NonNull::new(device_ptr).map(|raw| FfiDeviceImpl {
                    ops,
                    raw,
                    _phantom: PhantomData,
                })
            })
            .collect();

        Ok(Self {
            ops,
            devices_ptr,
            count,
            devices,
        })
    }

    /// 列挙されたデバイスのスライスを返す。
    pub fn devices(&self) -> &[FfiDeviceImpl<'static>] {
        &self.devices
    }

    /// macOS AVFoundation でデバイスを列挙する。
    #[cfg(target_os = "macos")]
    pub(crate) fn enumerate_avf() -> Result<Self> {
        Self::enumerate(&OPS_AVF)
    }

    /// Linux V4L2 でデバイスを列挙する。
    #[cfg(all(target_os = "linux", feature = "v4l2"))]
    pub(crate) fn enumerate_v4l2() -> Result<Self> {
        Self::enumerate(&OPS_V4L2)
    }

    /// Linux PipeWire でデバイスを列挙する。
    #[cfg(all(target_os = "linux", feature = "pipewire"))]
    pub(crate) fn enumerate_pipewire() -> Result<Self> {
        Self::enumerate(&OPS_PIPEWIRE)
    }
}

impl Drop for FfiDeviceListImpl {
    fn drop(&mut self) {
        if !self.devices_ptr.is_null() {
            // SAFETY: devices_ptr は enumerate_devices で確保された配列であり、
            // count はその要素数と一致する。本メソッドは一度だけ呼ばれる。
            unsafe { (self.ops.free_devices)(self.devices_ptr, self.count) };
        }
    }
}

// SAFETY: 内部の FFI ポインタ配列は read-only 参照であり、スレッド間共有は安全。
unsafe impl Send for FfiDeviceListImpl {}
unsafe impl Sync for FfiDeviceListImpl {}

// ---------------------------------------------------------------------------
// バックエンド別 OPS 定数
// ---------------------------------------------------------------------------

/// macOS AVFoundation 用の DeviceOps。
#[cfg(target_os = "macos")]
const OPS_AVF: DeviceOps = DeviceOps {
    device_name: ffi::video_avf_device_name,
    device_unique_id: ffi::video_avf_device_unique_id,
    device_format_count: ffi::video_avf_device_format_count,
    device_get_format: ffi::video_avf_device_get_format,
    enumerate_devices: ffi::video_avf_enumerate_devices,
    free_devices: ffi::video_avf_free_devices,
};

/// Linux V4L2 用の DeviceOps。
#[cfg(all(target_os = "linux", feature = "v4l2"))]
const OPS_V4L2: DeviceOps = DeviceOps {
    device_name: ffi::video_v4l2_device_name,
    device_unique_id: ffi::video_v4l2_device_unique_id,
    device_format_count: ffi::video_v4l2_device_format_count,
    device_get_format: ffi::video_v4l2_device_get_format,
    enumerate_devices: ffi::video_v4l2_enumerate_devices,
    free_devices: ffi::video_v4l2_free_devices,
};

/// Linux PipeWire 用の DeviceOps。
#[cfg(all(target_os = "linux", feature = "pipewire"))]
const OPS_PIPEWIRE: DeviceOps = DeviceOps {
    device_name: ffi::video_pipewire_device_name,
    device_unique_id: ffi::video_pipewire_device_unique_id,
    device_format_count: ffi::video_pipewire_device_format_count,
    device_get_format: ffi::video_pipewire_device_get_format,
    enumerate_devices: ffi::video_pipewire_enumerate_devices,
    free_devices: ffi::video_pipewire_free_devices,
};
