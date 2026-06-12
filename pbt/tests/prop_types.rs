use proptest::prelude::*;
use shiguredo_video_device::{PixelFormat, VideoFrameOwned};

fn arbitrary_pixel_format() -> impl Strategy<Value = PixelFormat> {
    prop_oneof![
        Just(PixelFormat::Nv12),
        Just(PixelFormat::Yuy2),
        Just(PixelFormat::I420),
        Just(PixelFormat::Mjpeg),
    ]
}

proptest! {
    #[test]
    fn to_raw_matches_expected_fourcc(pf in arbitrary_pixel_format()) {
        match pf {
            PixelFormat::Nv12 => assert_eq!(pf.to_raw(), 0x3231564E),
            PixelFormat::Yuy2 => assert_eq!(pf.to_raw(), 0x32595559),
            PixelFormat::I420 => assert_eq!(pf.to_raw(), 0x30323449),
            PixelFormat::Mjpeg => assert_eq!(pf.to_raw(), 0x47504A4D),
            PixelFormat::Unknown(_) => {}
        }
    }

    #[test]
    fn name_is_not_empty(pf in arbitrary_pixel_format()) {
        assert!(!pf.name().is_empty());
    }

    #[test]
    fn display_contains_name(pf in arbitrary_pixel_format()) {
        let display = format!("{pf}");
        assert!(display.contains(pf.name()), "display={display} name={}", pf.name());
    }
}

fn arbitrary_video_frame_owned() -> impl Strategy<Value = VideoFrameOwned> {
    let data = proptest::collection::vec(any::<u8>(), 0..1024);
    let pf = arbitrary_pixel_format();
    (data, pf).prop_map(|(data, pixel_format)| {
        let uv_data: Option<Vec<u8>> = match pixel_format {
            PixelFormat::Mjpeg => None,
            _ => Some(vec![0u8; 16]),
        };
        let stride: i32 = match pixel_format {
            PixelFormat::Mjpeg => 0,
            _ => 640,
        };
        let stride_uv: i32 = match pixel_format {
            PixelFormat::Mjpeg => 0,
            _ => 320,
        };
        VideoFrameOwned {
            data,
            uv_data,
            width: 640,
            height: 480,
            stride,
            stride_uv,
            pixel_format,
            timestamp_us: 0,
            pixel_buffer: None,
        }
    })
}

proptest! {
    #[test]
    fn video_frame_to_owned_roundtrip(owned in arbitrary_video_frame_owned()) {
        let frame = owned.as_frame();
        let roundtripped = frame.to_owned();
        assert_eq!(roundtripped.data, owned.data);
        assert_eq!(roundtripped.uv_data, owned.uv_data);
        assert_eq!(roundtripped.width, owned.width);
        assert_eq!(roundtripped.height, owned.height);
        assert_eq!(roundtripped.stride, owned.stride);
        assert_eq!(roundtripped.stride_uv, owned.stride_uv);
        assert_eq!(roundtripped.pixel_format, owned.pixel_format);
        assert_eq!(roundtripped.timestamp_us, owned.timestamp_us);
        assert_eq!(roundtripped.pixel_buffer, owned.pixel_buffer);
    }
}
