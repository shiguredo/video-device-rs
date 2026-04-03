//! ビデオデバイスの実機キャプチャテスト
//!
//! デバイスが接続された環境でのみ実行可能。
//! `cargo test -- --ignored` で実行する。

use std::sync::Once;
use std::sync::mpsc::sync_channel;
use std::time::Duration;

use shiguredo_video_device::{
    VideoCapture, VideoCaptureConfig, VideoDeviceList, VideoFrame, VideoFrameOwned,
};

/// デバイスが存在する環境で列挙が成功し、名前と ID が空でないことを確認する
#[test]
#[ignore]
fn test_enumerate_devices() {
    let device_list = VideoDeviceList::enumerate().expect("device enumeration failed");
    assert!(!device_list.is_empty(), "no video device found");

    for device in &device_list {
        let name = device.name().expect("failed to get device name");
        let id = device.unique_id().expect("failed to get device unique_id");
        assert!(!name.is_empty(), "device name is empty");
        assert!(!id.is_empty(), "device unique_id is empty");
    }
}

/// デバイスを掴んで 5 フレーム受信できることを確認する
#[test]
#[ignore]
fn test_capture_frames() {
    let target_frames = 5;
    let timeout = Duration::from_secs(10);

    // デバイスを列挙して先頭デバイスの ID を取得する
    let device_list = VideoDeviceList::enumerate().expect("device enumeration failed");
    assert!(!device_list.is_empty(), "no video device found");
    let device_id = device_list.devices()[0]
        .unique_id()
        .expect("failed to get device unique_id");

    // コールバックからメインスレッドへフレームを送るチャネル
    let (tx, rx) = sync_channel::<VideoFrameOwned>(1);

    let config = VideoCaptureConfig {
        device_id: Some(device_id),
        ..VideoCaptureConfig::default()
    };

    let mut capture = VideoCapture::new(config, move |frame: VideoFrame<'_>| {
        static DROP_LOG: Once = Once::new();
        if tx.try_send(frame.to_owned()).is_err() {
            DROP_LOG.call_once(|| {
                eprintln!(
                    "test_capture_frames: dropped frame (channel full or receiver disconnected)"
                );
            });
        }
    })
    .expect("VideoCapture creation failed");

    capture.start().expect("capture start failed");

    // 指定フレーム数を受信する
    for i in 0..target_frames {
        let owned = rx
            .recv_timeout(timeout)
            .unwrap_or_else(|_| panic!("timeout waiting for frame {}/{}", i + 1, target_frames));

        let frame = owned.as_frame();
        assert!(frame.width > 0, "frame width must be positive");
        assert!(frame.height > 0, "frame height must be positive");
        assert!(!frame.data.is_empty(), "frame data must not be empty");
        assert!(frame.stride > 0, "frame stride must be positive");
    }

    capture.stop();
}
