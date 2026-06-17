//! PipeWire バックエンドの統合テスト
//!
//! PipeWire デーモンとカメラが接続された環境でのみ実行可能。
//! `cargo test -- --ignored` で実行する。

#[cfg(enable_pipewire)]
mod pipewire_tests {
    use shiguredo_video_device::VideoDeviceList;

    // デバイス列挙が成功し、フォーマット情報が空でないことを確認する
    #[test]
    #[ignore]
    fn pipewire_enumerate_returns_formats() {
        // PipeWire デーモンが起動していて、少なくとも 1 台のカメラが接続されていることを期待する
        let list = VideoDeviceList::enumerate_pipewire()
            .expect("PipeWire デーモンが起動していて、少なくとも 1 台のカメラが接続されていること");
        assert!(
            !list.is_empty(),
            "少なくとも 1 台の Video/Source デバイスが列挙されること"
        );
        for device in &list {
            let name = device.name().expect("デバイス名が取得できること");
            assert!(!name.is_empty(), "デバイス名が空でないこと");

            let unique_id = device.unique_id().expect("unique_id が取得できること");
            assert!(!unique_id.is_empty(), "unique_id が空でないこと");

            let formats = device.formats();
            assert!(
                !formats.is_empty(),
                "デバイス '{name}' の format_count が 0 でないこと"
            );

            for format in &formats {
                assert!(format.width > 0, "width が正の値であること: {format:?}");
                assert!(format.height > 0, "height が正の値であること: {format:?}");
                assert!(
                    format.max_fps > 0.0,
                    "max_fps が正の値であること: {format:?}"
                );
                // pixel_format が NV12 / YUY2 / I420 のいずれかであること
                let fmt_name = format.pixel_format.name();
                assert!(
                    fmt_name == "NV12" || fmt_name == "YUY2" || fmt_name == "I420",
                    "pixel_format が NV12 / YUY2 / I420 のいずれかであること: {fmt_name}"
                );
            }
        }
    }

    // 複数デバイスがある場合に各デバイスの unique_id が重複していないことを確認する
    #[test]
    #[ignore]
    fn pipewire_multiple_devices_formats() {
        let list =
            VideoDeviceList::enumerate_pipewire().expect("PipeWire デーモンが起動していること");
        if list.is_empty() {
            return;
        }
        let ids: Vec<String> = list
            .as_slice()
            .iter()
            .map(|d| d.unique_id().unwrap_or_default())
            .collect();
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(
            ids.len(),
            unique_ids.len(),
            "unique_id が重複していないこと"
        );
    }

    // format_count と formats() の長さが一致することを確認する
    #[test]
    #[ignore]
    fn pipewire_format_index_bounds() {
        let list =
            VideoDeviceList::enumerate_pipewire().expect("PipeWire デーモンが起動していること");
        if list.is_empty() {
            return;
        }
        let device = &list.as_slice()[0];
        let count = device.format_count();
        let formats = device.formats();
        assert_eq!(
            count,
            formats.len(),
            "format_count と formats() の長さが一致すること"
        );
    }
}
