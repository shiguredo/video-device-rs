//! `src/error.rs` が定義する `Error` の Property-Based Testing。
//!
//! 全バリアントの `Display` 出力と `std::error::Error` 実装を検証する。

use std::cell::Cell;

use shiguredo_video_device::{Error, PixelFormat};

/// 各 PBT 共通のシード取得用環境変数名
///
/// 失敗時に表示される hex シードをそのまま代入して再現する。
const SEED_ENV: &str = "VIDEO_DEVICE_PBT_SEED";

/// デフォルトのケースバジェット
const CASES: usize = 256;

/// `Error` の全バリアントを生成する
///
/// 到達ゲートで全バリアントが実際に生成されたことを確認できるよう、
/// `sample_error` の戻り値と併せてバリアント番号も返す。
fn sample_error(ctx: &mut noprop::TestCaseContext) -> (Error, usize) {
    let index = noprop::sample_weighted_index(ctx, &[1; 10]);
    let err = match index {
        0 => Error::DeviceNotFound,
        1 => Error::DeviceAccessDenied,
        2 => Error::SessionCreateFailed,
        3 => Error::SessionStartFailed,
        4 => {
            // UnsupportedPixelFormat は全 PixelFormat を保持しうる
            let pf = match noprop::sample_weighted_index(ctx, &[1; 5]) {
                0 => PixelFormat::Nv12,
                1 => PixelFormat::Yuy2,
                2 => PixelFormat::I420,
                3 => PixelFormat::Mjpeg,
                _ => PixelFormat::Unknown(noprop::sample_u32(ctx)),
            };
            Error::UnsupportedPixelFormat(pf)
        }
        5 => Error::NullPointer("device name"),
        6 => Error::InvalidCaptureConfig("width, height, and fps must be positive integers"),
        7 => Error::ComInitFailed,
        8 => Error::InvalidUtf8("device name"),
        _ => Error::CaptureFaulted,
    };
    (err, index)
}

/// 全バリアントの `Display` 出力が空でない
#[test]
fn prop_error_display_is_non_empty() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    let reached = (0..10).map(|_| Cell::new(0usize)).collect::<Vec<_>>();
    let mut runner = noprop::Runner::new(seed);

    runner.run(CASES, |ctx| {
        let (err, index) = sample_error(ctx);
        let msg = err.to_string();
        assert!(!msg.is_empty(), "Display 出力が空でないこと: {err:?}");
        // 到達ゲートは Display の検証が成功した後に数える
        reached[index].set(reached[index].get() + 1);
        Ok(())
    })?;

    let unreached: Vec<usize> = (0..10).filter(|&i| reached[i].get() == 0).collect();
    assert!(
        unreached.is_empty(),
        "全バリアントの Display が検証されること (未到達: {unreached:?})\n{runner}"
    );
    Ok(())
}

/// 名前を持つバリアントの `Display` 出力に、その名前が反映される
#[test]
fn prop_named_variants_contain_value() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    let mut runner = noprop::Runner::new(seed);

    runner.run(CASES, |ctx| {
        let len = noprop::sample_usize_in(ctx, 1..=16);
        let name = noprop::sample_ascii_printable_string(ctx, len);
        let leaked: &'static str = Box::leak(name.clone().into_boxed_str());

        // NullPointer / InvalidUtf8 は接頭辞 + 名前
        let msg = Error::NullPointer(leaked).to_string();
        assert!(
            msg.starts_with("null pointer: "),
            "接頭辞が一致すること: {msg}"
        );
        assert!(msg.contains(&name), "名前が含まれること: {msg}");

        let msg = Error::InvalidUtf8(leaked).to_string();
        assert!(
            msg.starts_with("invalid UTF-8 in "),
            "接頭辞が一致すること: {msg}"
        );
        assert!(msg.contains(&name), "名前が含まれること: {msg}");

        // InvalidCaptureConfig は接頭辞 + メッセージ
        let msg = Error::InvalidCaptureConfig(leaked).to_string();
        assert!(
            msg.starts_with("invalid capture config: "),
            "接頭辞が一致すること: {msg}"
        );
        assert!(msg.contains(&name), "メッセージが含まれること: {msg}");
        Ok(())
    })?;
    Ok(())
}

/// `UnsupportedPixelFormat` の `Display` 出力にピクセルフォーマットが反映される
#[test]
fn prop_unsupported_pixel_format_contains_format() -> noprop::TestResult {
    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    let unknown = Cell::new(0usize);
    let mut runner = noprop::Runner::new(seed);

    runner.run(CASES, |ctx| {
        let pf = match noprop::sample_weighted_index(ctx, &[1; 5]) {
            0 => PixelFormat::Nv12,
            1 => PixelFormat::Yuy2,
            2 => PixelFormat::I420,
            3 => PixelFormat::Mjpeg,
            _ => {
                unknown.set(unknown.get() + 1);
                PixelFormat::Unknown(noprop::sample_u32(ctx))
            }
        };
        let msg = Error::UnsupportedPixelFormat(pf).to_string();
        assert!(
            msg.starts_with("unsupported pixel format: "),
            "接頭辞が一致すること: {msg}"
        );
        assert!(
            msg.contains(&pf.to_string()),
            "ピクセルフォーマットの Display が含まれること: {msg}"
        );
        Ok(())
    })?;

    assert!(
        unknown.get() > 0,
        "Unknown の検証が 1 件以上実行されること\n{runner}"
    );
    Ok(())
}

/// `Error::source()` は常に `None`
#[test]
fn prop_error_source_is_none() -> noprop::TestResult {
    use std::error::Error as StdError;

    let seed = noprop::seed_from_env_or_time(SEED_ENV)?;
    let mut runner = noprop::Runner::new(seed);

    runner.run(CASES, |ctx| {
        let (err, _) = sample_error(ctx);
        assert!(err.source().is_none(), "source() は常に None を返すこと");
        Ok(())
    })?;
    Ok(())
}
