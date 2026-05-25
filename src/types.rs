use std::ffi::c_void;
use std::fmt;
use std::sync::atomic::AtomicBool;

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn CFRetain(cf: *const c_void) -> *const c_void;
    fn CFRelease(cf: *const c_void);
}

/// ピクセルフォーマット定数 (video_c.h と同じ値)
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) const VIDEO_PIXEL_FORMAT_NV12: u32 = 0x3231564E;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) const VIDEO_PIXEL_FORMAT_YUY2: u32 = 0x32595559;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) const VIDEO_PIXEL_FORMAT_I420: u32 = 0x30323449;

/// ピクセルフォーマット
///
/// **Windows** では `to_raw` / `from_raw` はビルド対象に含まれない（`cfg` により定義されない）。
/// 列挙・キャプチャは Media Foundation の `GUID` と内部で対応付けている。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// NV12 (YUV 4:2:0 semi-planar)
    Nv12,
    /// YUY2 (YUV 4:2:2 packed)
    Yuy2,
    /// I420 (YUV 4:2:0 planar)
    I420,
    /// 不明なフォーマット
    Unknown(u32),
}

impl PixelFormat {
    /// 生の値からピクセルフォーマットを生成
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn from_raw(raw: u32) -> Self {
        match raw {
            VIDEO_PIXEL_FORMAT_NV12 => PixelFormat::Nv12,
            VIDEO_PIXEL_FORMAT_YUY2 => PixelFormat::Yuy2,
            VIDEO_PIXEL_FORMAT_I420 => PixelFormat::I420,
            _ => PixelFormat::Unknown(raw),
        }
    }

    /// ピクセルフォーマットを生の値に変換
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn to_raw(&self) -> u32 {
        match self {
            PixelFormat::Nv12 => VIDEO_PIXEL_FORMAT_NV12,
            PixelFormat::Yuy2 => VIDEO_PIXEL_FORMAT_YUY2,
            PixelFormat::I420 => VIDEO_PIXEL_FORMAT_I420,
            PixelFormat::Unknown(raw) => *raw,
        }
    }

    /// フォーマット名を取得
    pub fn name(&self) -> &'static str {
        match self {
            PixelFormat::Nv12 => "NV12",
            PixelFormat::Yuy2 => "YUY2",
            PixelFormat::I420 => "I420",
            PixelFormat::Unknown(_) => "Unknown",
        }
    }
}

impl fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PixelFormat::Unknown(raw) => write!(f, "Unknown(0x{raw:08X})"),
            _ => f.write_str(self.name()),
        }
    }
}

/// macOS の CVPixelBuffer へのオペーク参照
///
/// Clone で retain、Drop で release する。
/// **macOS 以外**では C からは常に NULL が渡る想定であり、非 NULL の `from_retained_ptr` 取り込みは行わない（サポート外）。
#[derive(Default)]
pub struct PixelBuffer {
    ptr: *mut c_void,
}

impl PixelBuffer {
    /// 生ポインタを取得する
    pub fn as_ptr(&self) -> *mut c_void {
        self.ptr
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) unsafe fn from_retained_ptr(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() {
            return None;
        }
        #[cfg(target_os = "macos")]
        {
            Some(Self { ptr })
        }
        #[cfg(not(target_os = "macos"))]
        {
            // Linux では Drop で CFRelease しないため、非 NULL を保持するとリークしうる。契約上 NULL のみ。
            None
        }
    }
}

impl Clone for PixelBuffer {
    fn clone(&self) -> Self {
        #[cfg(target_os = "macos")]
        unsafe {
            if !self.ptr.is_null() {
                let _ = CFRetain(self.ptr.cast_const());
            }
        }

        Self { ptr: self.ptr }
    }
}

impl Drop for PixelBuffer {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        unsafe {
            if !self.ptr.is_null() {
                CFRelease(self.ptr.cast_const());
            }
        }
    }
}

impl fmt::Debug for PixelBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PixelBuffer")
            .field("ptr", &self.ptr)
            .finish()
    }
}

impl PartialEq for PixelBuffer {
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}

impl Eq for PixelBuffer {}

// Core Foundation の参照カウントはスレッドセーフで、保持しているのは不透明ポインタのみ。
unsafe impl Send for PixelBuffer {}

/// ビデオデバイスが対応するフォーマット
#[derive(Debug, Clone)]
pub struct VideoFormat {
    /// 幅
    pub width: i32,
    /// 高さ
    pub height: i32,
    /// 最小フレームレート
    pub min_fps: f32,
    /// 最大フレームレート
    pub max_fps: f32,
    /// ピクセルフォーマット
    pub pixel_format: PixelFormat,
}

/// キャプチャ設定
///
/// **Windows** では `width` / `height` / `fps` はいずれも正の整数である必要がある（Media Foundation への渡し方のため）。
/// **Linux（PipeWire 等）** では不正値を C 側が既定解像度・フレームレートに置き換える場合があるため、Rust 側では拒否しない。
///
/// ネゴシエーション結果が未知のピクセルフォーマットになる場合の挙動は、バックエンド（macOS / V4L2 / PipeWire / Windows）により異なりうる。
pub struct VideoCaptureConfig {
    /// デバイス ID (None の場合はデフォルトデバイス)
    pub device_id: Option<String>,
    /// 幅
    pub width: i32,
    /// 高さ
    pub height: i32,
    /// フレームレート
    pub fps: i32,
    /// 取得するピクセルフォーマット (None の場合はデフォルト選択)
    pub pixel_format: Option<PixelFormat>,
}

impl Default for VideoCaptureConfig {
    fn default() -> Self {
        Self {
            device_id: None,
            width: 640,
            height: 480,
            fps: 30,
            pixel_format: None,
        }
    }
}

/// キャプチャされたビデオフレームの生データ（借用）。
///
/// `data` および `uv_data` が指すメモリの寿命は、ユーザに渡したコールバックの呼び出し中に限る。
pub struct VideoFrame<'a> {
    /// Y プレーンまたはインターリーブデータ
    pub data: &'a [u8],
    /// UV プレーン (NV12/I420 の場合のみ、YUY2 では None)
    pub uv_data: Option<&'a [u8]>,
    /// 幅 (ピクセル)
    pub width: i32,
    /// 高さ (ピクセル)
    pub height: i32,
    /// data のストライド (バイト/行)
    pub stride: i32,
    /// uv_data のストライド (NV12/I420 の場合のみ)
    pub stride_uv: i32,
    /// ピクセルフォーマット
    pub pixel_format: PixelFormat,
    /// タイムスタンプ (マイクロ秒)
    pub timestamp_us: i64,
    /// CVPixelBuffer へのオペーク参照（**macOS のみ**。Linux では C から NULL のみ）
    pub pixel_buffer: Option<PixelBuffer>,
}

impl<'a> VideoFrame<'a> {
    /// 参照データを所有データに変換する
    pub fn to_owned(&self) -> VideoFrameOwned {
        VideoFrameOwned {
            data: self.data.to_vec(),
            uv_data: self.uv_data.map(|data| data.to_vec()),
            width: self.width,
            height: self.height,
            stride: self.stride,
            stride_uv: self.stride_uv,
            pixel_format: self.pixel_format,
            timestamp_us: self.timestamp_us,
            pixel_buffer: self.pixel_buffer.clone(),
        }
    }
}

/// キャプチャされたビデオフレームの所有データ。
///
/// コールバック終了後も保持でき、別スレッドへ渡すなど寿命を延ばす用途はこちらを使う（[`VideoFrame::to_owned`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrameOwned {
    /// Y プレーンまたはインターリーブデータ
    pub data: Vec<u8>,
    /// UV プレーン (NV12/I420 の場合のみ、YUY2 では None)
    pub uv_data: Option<Vec<u8>>,
    /// 幅 (ピクセル)
    pub width: i32,
    /// 高さ (ピクセル)
    pub height: i32,
    /// data のストライド (バイト/行)
    pub stride: i32,
    /// uv_data のストライド (NV12/I420 の場合のみ)
    pub stride_uv: i32,
    /// ピクセルフォーマット
    pub pixel_format: PixelFormat,
    /// タイムスタンプ (マイクロ秒)
    pub timestamp_us: i64,
    /// CVPixelBuffer へのオペーク参照（**macOS のみ**。Linux では C から NULL のみ）
    pub pixel_buffer: Option<PixelBuffer>,
}

impl VideoFrameOwned {
    /// 参照フレームとして取得する
    pub fn as_frame(&self) -> VideoFrame<'_> {
        VideoFrame {
            data: &self.data,
            uv_data: self.uv_data.as_deref(),
            width: self.width,
            height: self.height,
            stride: self.stride,
            stride_uv: self.stride_uv,
            pixel_format: self.pixel_format,
            timestamp_us: self.timestamp_us,
            pixel_buffer: self.pixel_buffer.clone(),
        }
    }
}

pub(crate) struct CaptureContext {
    pub(crate) callback: Box<dyn Fn(VideoFrame<'_>) + Send>,
    pub(crate) running: AtomicBool,
}
