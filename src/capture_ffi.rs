//! macOS / Linux 共通のビデオキャプチャ実装。
//!
//! バックエンドごとに FFI 関数群を [`CaptureOps`] で渡し、`CaptureInner` が
//! `VideoCapture` の実装を一括で提供する。各プラットフォームファイルは [`CaptureOps`] の
//! 定数定義と newtype ラッパーのみを持つ。

use std::ffi::{CString, c_char, c_void};
use std::ptr::NonNull;

use crate::VideoCapture;
use crate::error::{Error, Result};
use crate::ffi;
use crate::frame_math;
use crate::types::{PixelBuffer, PixelFormat, VideoCaptureConfig, VideoFrame};

// ---------------------------------------------------------------------------
// バックエンドとの境界
// ---------------------------------------------------------------------------

/// バックエンド固有の FFI 関数テーブル。
///
/// 各プラットフォームファイル（`capture_avf.rs` 等）で `const` として
/// 1 つだけ定義し、`CaptureInner` に `&'static` で渡す。
pub(crate) struct CaptureOps {
    /// キャプチャセッションを生成する。
    pub session_create: unsafe extern "C" fn(
        device_id: *const c_char,
        width: i32,
        height: i32,
        fps: i32,
        pixel_format: u32,
    ) -> *mut ffi::VideoSession,
    /// キャプチャを開始する。`callback` には `frame_callback` を常に渡す。
    pub session_start: unsafe extern "C" fn(
        session: *mut ffi::VideoSession,
        callback: ffi::FrameCallback,
        context: *mut c_void,
    ) -> i32,
    /// キャプチャを停止する。
    pub session_stop: unsafe extern "C" fn(session: *mut ffi::VideoSession),
    /// キャプチャセッションを破棄する。
    pub session_destroy: unsafe extern "C" fn(session: *mut ffi::VideoSession),
}

// ---------------------------------------------------------------------------
// コールバックコンテキスト
// ---------------------------------------------------------------------------

/// キャプチャスレッドからユーザーコールバックを呼び出すためのコンテキスト。
///
/// `user_data` として C に渡される `Box<CaptureContext>` のポインタ経由で
/// `frame_callback` からアクセスされる。
pub(crate) struct CaptureContext {
    /// ユーザーが登録したフレームコールバック。
    pub callback: Box<dyn Fn(VideoFrame<'_>) + Send + 'static>,
}

// ---------------------------------------------------------------------------
// 汎用キャプチャ実装
// ---------------------------------------------------------------------------

/// バックエンド非依存のキャプチャ実装。
///
/// すべての FFI 呼び出しは `ops` に格納された関数ポインタ経由で行われる。
pub(crate) struct CaptureInner {
    /// バックエンドの FFI 関数テーブル。
    ops: &'static CaptureOps,
    /// キャプチャセッション（`new()` で生成し `Drop` で破棄）。
    session: Option<NonNull<ffi::VideoSession>>,
    /// コールバックコンテキスト。
    context: Option<Box<CaptureContext>>,
    /// キャプチャ設定。
    config: VideoCaptureConfig,
    /// キャプチャが実行中かどうかのフラグ。
    running: bool,
}

impl CaptureInner {
    /// キャプチャを構築する。
    ///
    /// この時点ではキャプチャスレッドは起動せず、`start()` が呼ばれるまで待機する。
    pub fn new<F>(ops: &'static CaptureOps, config: VideoCaptureConfig, callback: F) -> Result<Self>
    where
        F: Fn(VideoFrame<'_>) + Send + 'static,
    {
        // 不明なピクセルフォーマットが指定された場合は早期に拒否する
        let requested_pixel_format = match config.pixel_format {
            Some(pixel_format @ PixelFormat::Unknown(_)) => {
                return Err(Error::UnsupportedPixelFormat(pixel_format));
            }
            Some(pixel_format) => pixel_format.to_raw(),
            None => 0,
        };

        // device_id を C 文字列に変換する（NULL バイトを含む場合は拒否）
        let device_id_cstr = config.device_id.as_ref().map(|s| CString::new(s.as_str()));
        let device_id_ptr = match &device_id_cstr {
            Some(Ok(cstr)) => cstr.as_ptr(),
            Some(Err(_)) => {
                return Err(Error::NullPointer("device_id contains null byte"));
            }
            None => std::ptr::null(),
        };

        // SAFETY: FFI 呼び出し。device_id_ptr は CString の有効なポインタまたは NULL。
        // config の width/height/fps はユーザー指定値であり、C 側で検証済み。
        let session = unsafe {
            (ops.session_create)(
                device_id_ptr,
                config.width,
                config.height,
                config.fps,
                requested_pixel_format,
            )
        };

        let session = NonNull::new(session).ok_or(Error::SessionCreateFailed)?;

        let context = Box::new(CaptureContext {
            callback: Box::new(callback),
        });

        Ok(Self {
            ops,
            session: Some(session),
            context: Some(context),
            config,
            running: false,
        })
    }
}

impl VideoCapture for CaptureInner {
    fn start(&mut self) -> Result<()> {
        let session = self.session.ok_or(Error::SessionStartFailed)?;
        let context = self.context.as_mut().ok_or(Error::SessionStartFailed)?;

        if self.running {
            return Ok(());
        }

        let context_ptr = &mut **context as *mut CaptureContext as *mut c_void;

        // SAFETY: session は NonNull で保証された有効なポインタ。
        // context_ptr は Box<CaptureContext> から取得したポインタであり、
        // CaptureInner のライフタイムの間は有効。
        let ret = unsafe {
            (self.ops.session_start)(session.as_ptr(), Some(frame_callback), context_ptr)
        };

        if ret < 0 {
            return Err(Error::SessionStartFailed);
        }

        self.running = true;
        Ok(())
    }

    fn stop(&mut self) {
        if self.running {
            if let Some(session) = self.session {
                // SAFETY: session は start() 呼び出し時に作成された有効なポインタ。
                unsafe { (self.ops.session_stop)(session.as_ptr()) };
            }
            self.running = false;
        }
    }

    fn config(&self) -> &VideoCaptureConfig {
        &self.config
    }
}

impl Drop for CaptureInner {
    fn drop(&mut self) {
        self.stop();
        if let Some(session) = self.session.take() {
            // SAFETY: session は new() で作成され、まだ破棄されていない。
            unsafe { (self.ops.session_destroy)(session.as_ptr()) };
        }
    }
}

// SAFETY: 内部に保持する FFI セッションポインタは C 側のスレッド安全性に従い、
// CaptureContext のコールバックは Box<dyn Fn + Send> でスレッド安全。
// 全バックエンドでキャプチャは専用スレッド上で動作するため、本構造体の
// スレッド間移動は安全。
unsafe impl Send for CaptureInner {}

// ---------------------------------------------------------------------------
// extern "C" フレームコールバック（全バックエンド共通）
// ---------------------------------------------------------------------------

/// C 側からフレームデータを受け取り、ユーザーの Rust コールバックに橋渡しする。
///
/// `user_data` には `CaptureContext` へのポインタが渡される。
extern "C" fn frame_callback(
    user_data: *mut c_void,
    data: *const u8,
    uv_data: *const u8,
    width: i32,
    height: i32,
    stride: i32,
    stride_uv: i32,
    pixel_format: u32,
    timestamp_us: i64,
    pixel_buffer: *mut c_void,
) {
    let pixel_buffer = unsafe { PixelBuffer::from_retained_ptr(pixel_buffer) };

    // 無効な引数を持つフレームは無視する
    if user_data.is_null() || data.is_null() || width <= 0 || height <= 0 {
        return;
    }

    // SAFETY: user_data は CaptureInner::start() で Box<CaptureContext> から
    // 取得したポインタであり、CaptureInner のライフタイム中は有効。
    let context = unsafe { &*(user_data as *const CaptureContext) };

    let pf = PixelFormat::from_raw(pixel_format);

    let frame = match pf {
        PixelFormat::Nv12 => {
            if uv_data.is_null() {
                return;
            }
            let Some((y_size, uv_size)) = frame_math::nv12_plane_sizes(stride, stride_uv, height)
            else {
                return;
            };

            // SAFETY: C 側が保証する有効なデータ領域。
            // 領域長は nv12_plane_sizes で計算済み。
            let y_slice = unsafe { std::slice::from_raw_parts(data, y_size) };
            let uv_slice = unsafe { std::slice::from_raw_parts(uv_data, uv_size) };

            VideoFrame {
                data: y_slice,
                uv_data: Some(uv_slice),
                width,
                height,
                stride,
                stride_uv,
                pixel_format: pf,
                timestamp_us,
                pixel_buffer,
            }
        }
        PixelFormat::I420 => {
            if uv_data.is_null() {
                return;
            }
            let Some((y_size, uv_size)) = frame_math::i420_plane_sizes(stride, stride_uv, height)
            else {
                return;
            };

            // SAFETY: C 側が保証する有効なデータ領域。
            let y_slice = unsafe { std::slice::from_raw_parts(data, y_size) };
            let uv_slice = unsafe { std::slice::from_raw_parts(uv_data, uv_size) };

            VideoFrame {
                data: y_slice,
                uv_data: Some(uv_slice),
                width,
                height,
                stride,
                stride_uv,
                pixel_format: pf,
                timestamp_us,
                pixel_buffer,
            }
        }
        PixelFormat::Yuy2 => {
            let Some(data_size) = frame_math::yuy2_packed_frame_bytes(stride, height) else {
                return;
            };

            // SAFETY: C 側が保証する有効なデータ領域。
            let data_slice = unsafe { std::slice::from_raw_parts(data, data_size) };

            VideoFrame {
                data: data_slice,
                uv_data: None,
                width,
                height,
                stride,
                stride_uv: 0,
                pixel_format: pf,
                timestamp_us,
                pixel_buffer,
            }
        }
        PixelFormat::Unknown(_) => return,
    };

    (context.callback)(frame);
}
