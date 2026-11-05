//! ビデオデバイスの実機キャプチャテスト
//!
//! デバイスが接続された環境でのみ実行する。
//! CI の Device Test ジョブが設定する環境変数 `VIDEO_DEVICE_DEVICE_TEST` がある場合だけ
//! 実際の検証を行い、未設定の環境ではスキップする。
//!
//! バックエンドが 1 つも有効でないビルド (PBT の default-features = false など)
//! ではコンパイル対象から外す。

#![cfg(any(enable_avf, enable_v4l2, enable_pipewire, enable_mf))]

use std::sync::Once;
use std::sync::mpsc::sync_channel;
use std::time::Duration;

#[path = "helpers/device_test.rs"]
mod device_test;

use shiguredo_video_device::{
    VideoCapture, VideoCaptureConfig, VideoDeviceList, VideoFrame, VideoFrameOwned,
};

/// デバイスが存在する環境で列挙が成功し、名前と ID が空でないことを確認する
#[test]
fn test_enumerate_devices() {
    if !device_test::device_test_enabled() {
        eprintln!("test_enumerate_devices: VIDEO_DEVICE_DEVICE_TEST が未設定のためスキップします");
        return;
    }

    let device_list = VideoDeviceList::enumerate().expect("デバイスの列挙に失敗しました");

    assert!(
        !device_list.is_empty(),
        "1 台以上のビデオデバイスが存在すること"
    );

    for device in &device_list {
        let name = device.name().expect("デバイス名の取得に失敗しました");
        let id = device.unique_id().expect("unique_id の取得に失敗しました");
        assert!(!name.is_empty(), "デバイス名が空でないこと");
        assert!(!id.is_empty(), "unique_id が空でないこと");
    }
}

/// デバイスを掴んで 5 フレーム受信できることを確認する
#[test]
fn test_capture_frames() {
    if !device_test::device_test_enabled() {
        eprintln!("test_capture_frames: VIDEO_DEVICE_DEVICE_TEST が未設定のためスキップします");
        return;
    }

    let target_frames = 5;
    let timeout = Duration::from_secs(10);

    // デバイスを列挙して先頭デバイスの ID を取得する
    let device_list = VideoDeviceList::enumerate().expect("デバイスの列挙に失敗しました");

    assert!(
        !device_list.is_empty(),
        "1 台以上のビデオデバイスが存在すること"
    );
    let device_id = device_list.as_slice()[0]
        .unique_id()
        .expect("unique_id の取得に失敗しました");

    // コールバックからメインスレッドへフレームを送るチャネル
    let (tx, rx) = sync_channel::<VideoFrameOwned>(1);

    let config = VideoCaptureConfig {
        device_id: Some(device_id),
        ..VideoCaptureConfig::default()
    };

    let mut capture = VideoCapture::new(config, move |frame: VideoFrame<'_>| {
        send_frame(&tx, frame);
    })
    .expect("VideoCapture の作成に失敗しました");

    capture.start().expect("キャプチャの開始に失敗しました");

    // 指定フレーム数を受信する
    for i in 0..target_frames {
        let owned = rx.recv_timeout(timeout).unwrap_or_else(|_| {
            panic!(
                "フレーム {}/{} の受信がタイムアウトしました",
                i + 1,
                target_frames
            )
        });

        let frame = owned.as_frame();
        assert!(frame.width > 0, "フレームの幅が正の値であること");
        assert!(frame.height > 0, "フレームの高さが正の値であること");
        assert!(!frame.data.is_empty(), "フレームの data が空でないこと");
        assert!(frame.stride > 0, "フレームの stride が正の値であること");
    }

    capture.stop();
}

fn send_frame(tx: &std::sync::mpsc::SyncSender<VideoFrameOwned>, frame: VideoFrame<'_>) {
    static DROP_LOG: Once = Once::new();
    if tx.try_send(frame.to_owned()).is_err() {
        DROP_LOG.call_once(|| {
            eprintln!(
                "test_capture_frames: フレームを破棄しました (チャネル満杯または受信側の切断)"
            );
        });
    }
}
