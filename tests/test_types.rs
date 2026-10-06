//! `src/types.rs` の公開 API に対する単体テスト。
//!
//! `VideoFrame` / `VideoFrameOwned` のラウンドトリップは
//! `pbt/tests/prop_types.rs` の PBT で検証するため、ここでは扱わない。

use shiguredo_video_device::{PixelFormat, VideoCaptureConfig};

#[test]
fn default_capture_config_uses_automatic_pixel_format_selection() {
    let config = VideoCaptureConfig::default();

    assert!(config.pixel_format.is_none());
}

#[test]
fn capture_config_accepts_an_explicit_pixel_format() {
    let config = VideoCaptureConfig {
        pixel_format: Some(PixelFormat::Yuy2),
        ..VideoCaptureConfig::default()
    };

    assert_eq!(config.pixel_format, Some(PixelFormat::Yuy2));
}
