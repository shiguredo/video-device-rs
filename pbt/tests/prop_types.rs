//! `src/types.rs` が定義する公開型の Property-Based Testing。
//!
//! `PixelFormat` の `to_raw` / `name` / `Display` と、
//! `VideoFrameOwned` の `as_frame` / `VideoFrame::to_owned` のラウンドトリップを検証する。

use std::cell::Cell;

use shiguredo_video_device::{PixelFormat, VideoFrameOwned};

/// 各 PBT 共通のシード取得用環境変数名
///
/// 失敗時に表示される hex シードをそのまま代入して再現する。
const SEED_ENV: &str = "VIDEO_DEVICE_PBT_SEED";

/// デフォルトのケースバジェット
const CASES: usize = 256;

/// ピクセルフォーマットを一様に生成する
///
/// `Unknown` も含めて 5 種類すべてが同確率で選ばれる。
fn sample_pixel_format(ctx: &mut noprop::TestCaseContext) -> PixelFormat {
    match noprop::sample_weighted_index(ctx, &[1; 5]) {
        0 => PixelFormat::Nv12,
        1 => PixelFormat::Yuy2,
        2 => PixelFormat::I420,
        3 => PixelFormat::Mjpeg,
        _ => PixelFormat::Unknown(noprop::sample_u32(ctx)),
    }
}

/// `to_raw` が `video.h` の FourCC 定数と一致し、`Unknown` は生の値を保つ
#[test]
fn prop_to_raw_matches_expected_fourcc() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    let unknown = Cell::new(0usize);
    let mut runner = noprop::Runner::new(seed);

    runner.run(CASES, |ctx| {
        let pf = sample_pixel_format(ctx);
        let raw = pf.to_raw();
        match pf {
            PixelFormat::Nv12 => assert_eq!(raw, 0x3231564E, "NV12 の FourCC が一致すること"),
            PixelFormat::Yuy2 => assert_eq!(raw, 0x32595559, "YUY2 の FourCC が一致すること"),
            PixelFormat::I420 => assert_eq!(raw, 0x30323449, "I420 の FourCC が一致すること"),
            PixelFormat::Mjpeg => assert_eq!(raw, 0x47504A4D, "MJPEG の FourCC が一致すること"),
            PixelFormat::Unknown(value) => {
                assert_eq!(raw, value, "Unknown は生の値をそのまま返すこと");
                unknown.set(unknown.get() + 1);
            }
        }
        Ok(())
    })?;

    assert!(
        unknown.get() > 0,
        "Unknown の検証が 1 件以上実行されること\n{runner}"
    );
    Ok(())
}

/// `name` が既知のフォーマット名を返し、`Display` がその名前を含む
#[test]
fn prop_name_and_display_are_consistent() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    let unknown = Cell::new(0usize);
    let mut runner = noprop::Runner::new(seed);

    runner.run(CASES, |ctx| {
        let pf = sample_pixel_format(ctx);
        let expected = match pf {
            PixelFormat::Nv12 => "NV12",
            PixelFormat::Yuy2 => "YUY2",
            PixelFormat::I420 => "I420",
            PixelFormat::Mjpeg => "MJPEG",
            PixelFormat::Unknown(_) => "Unknown",
        };
        assert_eq!(pf.name(), expected, "name が期待値と一致すること");

        let display = format!("{pf}");
        assert!(
            display.contains(pf.name()),
            "Display が name を含むこと: display={display}"
        );
        if let PixelFormat::Unknown(raw) = pf {
            // Unknown は 16 進 8 桁の生の値を Display に含める
            assert!(
                display.contains(&format!("0x{raw:08X}")),
                "Display が生の値を含むこと: display={display}"
            );
            unknown.set(unknown.get() + 1);
        }
        Ok(())
    })?;

    assert!(
        unknown.get() > 0,
        "Unknown の検証が 1 件以上実行されること\n{runner}"
    );
    Ok(())
}

/// ラウンドトリップ検証用のフレームを生成する
///
/// フィールドは実フレームの契約から外れた値 (負の寸法など) も含めて生成し、
/// `as_frame` / `to_owned` が値の加工をしないことを確かめる。
fn sample_frame_owned(ctx: &mut noprop::TestCaseContext) -> VideoFrameOwned {
    let data = {
        let len = noprop::sample_usize_in(ctx, 0..=1024);
        noprop::sample_bytes_vec(ctx, len)
    };
    let uv_data = noprop::sample_bool(ctx).then(|| {
        let len = noprop::sample_usize_in(ctx, 0..=1024);
        noprop::sample_bytes_vec(ctx, len)
    });

    VideoFrameOwned {
        data,
        uv_data,
        width: noprop::sample_i32(ctx),
        height: noprop::sample_i32(ctx),
        stride: noprop::sample_i32(ctx),
        stride_uv: noprop::sample_i32(ctx),
        pixel_format: sample_pixel_format(ctx),
        timestamp_us: noprop::sample_i64(ctx),
        pixel_buffer: None,
    }
}

/// `VideoFrameOwned::as_frame` と `VideoFrame::to_owned` のラウンドトリップ
#[test]
fn prop_frame_owned_roundtrip() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    let with_uv = Cell::new(0usize);
    let without_uv = Cell::new(0usize);
    let mut runner = noprop::Runner::new(seed);

    runner.run(CASES, |ctx| {
        let owned = sample_frame_owned(ctx);
        let roundtrip = owned.as_frame().to_owned();

        assert_eq!(roundtrip.data, owned.data, "data が保持されること");
        assert_eq!(roundtrip.uv_data, owned.uv_data, "uv_data が保持されること");
        assert_eq!(roundtrip.width, owned.width, "width が保持されること");
        assert_eq!(roundtrip.height, owned.height, "height が保持されること");
        assert_eq!(roundtrip.stride, owned.stride, "stride が保持されること");
        assert_eq!(
            roundtrip.stride_uv, owned.stride_uv,
            "stride_uv が保持されること"
        );
        assert_eq!(
            roundtrip.pixel_format, owned.pixel_format,
            "pixel_format が保持されること"
        );
        assert_eq!(
            roundtrip.timestamp_us, owned.timestamp_us,
            "timestamp_us が保持されること"
        );
        assert_eq!(
            roundtrip.pixel_buffer, owned.pixel_buffer,
            "pixel_buffer が保持されること"
        );

        // uv_data あり / なしの両方の経路が実行されたことを数える
        if roundtrip.uv_data.is_some() {
            with_uv.set(with_uv.get() + 1);
        } else {
            without_uv.set(without_uv.get() + 1);
        }
        Ok(())
    })?;

    assert!(
        with_uv.get() > 0 && without_uv.get() > 0,
        "uv_data あり / なしの両方を検証すること (あり={}, なし={})\n{runner}",
        with_uv.get(),
        without_uv.get()
    );
    Ok(())
}
