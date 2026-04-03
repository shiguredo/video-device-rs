use shiguredo_video_device::{PixelFormat, VideoCaptureConfig, VideoFrame, VideoFrameOwned};

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

#[test]
fn frame_to_owned_preserves_absent_pixel_buffer() {
    let data = [0u8; 8];
    let uv = [0u8; 4];
    let frame = VideoFrame {
        data: &data,
        uv_data: Some(&uv),
        width: 2,
        height: 2,
        stride: 2,
        stride_uv: 2,
        pixel_format: PixelFormat::Nv12,
        timestamp_us: 42,
        pixel_buffer: None,
    };

    let owned = frame.to_owned();

    assert!(owned.pixel_buffer.is_none());
    assert_eq!(owned.pixel_format, PixelFormat::Nv12);
}

#[test]
fn owned_frame_as_frame_preserves_absent_pixel_buffer() {
    let owned = VideoFrameOwned {
        data: vec![0; 8],
        uv_data: Some(vec![0; 4]),
        width: 2,
        height: 2,
        stride: 2,
        stride_uv: 2,
        pixel_format: PixelFormat::Nv12,
        timestamp_us: 42,
        pixel_buffer: None,
    };

    let frame = owned.as_frame();

    assert!(frame.pixel_buffer.is_none());
    assert_eq!(frame.pixel_format, PixelFormat::Nv12);
}
