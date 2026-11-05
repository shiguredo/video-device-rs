# 実機キャプチャ・エラー経路・装置不要バリデーションのテストを拡充する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/add-test-strategy-expansion
- Polished: {YYYY-MM-DD}

## 目的

テスト戦略に次の穴がある。

1. `tests/test_capture.rs` は既定 `VideoCaptureConfig` での start → 5 フレーム受信 → stop のみを検証する。`pixel_format` を明示したキャプチャ、I420 / YUY2 / MJPEG、`stop()` 後の再 `start()`、二重 `stop()`、存在しない `device_id`、複数デバイス、start しないままの drop を検証していない。受信フレームも width / height / stride の正値と data 非空のみで、`data.len()` と stride × 高さの整合を見ていない
2. `pixel_format` を明示しないため、`frame_callback` の YUY2 / I420 分岐と `frame_math` の該当関数は実機で一度も実行されない。フォーマット指定の欠落は、C と Rust のフレーム長契約の不一致を検出できないことを意味する
3. macOS の実キャプチャは CI で明示的にスキップされ、`VIDEO_DEVICE_DEVICE_TEST` 未設定時はテストが早期 return して「ok」と表示される。未実行であることが CI の緑から判別できない
4. ハードウェア不要のバリデーションが未テスト。`PixelFormat::Unknown` の拒否、NUL バイトを含む `device_id` の拒否、AVF / PipeWire での `PixelFormat::Mjpeg` 拒否、Windows の `validate_capture_config_for_windows` は公開 API から到達でき、通常の CI で検証できる
5. `frame_math` の `checked_mul` オーバーフロー境界（`i32::MAX` 近傍）が未テスト
6. `tests/test_pipewire.rs` は `format_count == formats.len()` を assert するが、公開 doc は一致を保証していない。`unique_id()` のエラーを `unwrap_or_default()` で握りつぶしている。`pipewire_multiple_devices_formats` は実際には unique_id の重複を検証しており名前と内容が一致しない。`formats` が空の Video/Source ノードを許容しない
7. `tests/test_types.rs` の `capture_config_accepts_an_explicit_pixel_format` は構造体を作って同じ値を assert するだけで実装の写経。`frame_math` の `mjpeg_payload_bytes_returns_input` は入力をそのまま返すことの確認のみ
8. PBT の `SEED_ENV` / `CASES` / PixcelFormat サンプラーが `prop_types.rs` と `prop_error.rs` で重複している

なお、frame_math の PBT 追加は既存の PBT カバレッジ拡充 issue が追跡しているため、本 issue では単体テストの境界値のみを扱う。

## 優先度根拠

- Medium。C / Rust 間のフレーム長契約の不一致や状態遷移のバグは、実機テストとバリデーションテストでしか検出できない
- `/review-code` の重要指摘として 3 周にわたり確認済み

## 現状

- `tests/test_capture.rs` の設定は `..VideoCaptureConfig::default()` で `pixel_format: None`
- `cargo test --workspace` で device test は環境変数未設定時に早期 return する
- `tests/test_pipewire.rs` は列挙のみを検証する

## 設計方針

- 実機テストを拡張する。フォーマット指定のキャプチャはデバイスが対応するフォーマットを `formats()` から引き、対応時のみ実行する
- start → stop → start → フレーム再受信を検証する
- 受信フレームで `data.len()` と `stride * height`、UV スライスと `stride_uv * ceil(height / 2)` の整合を確認する
- 装置不要のバリデーションは feature ごとの `#[cfg]` を付けた integration test として追加する
- `frame_math` のオーバーフロー境界を単体テストに追加する
- `tests/test_pipewire.rs` の assert を公開契約に合わせ、命名とエラー処理を修正する
- 弱いテストを削除または意味のある検証に置き換える
- PBT の共通定数・サンプラーを `pbt/tests/helpers/` に集約する
- macOS の実キャプチャを実行できない場合の手動検証手順を docs に残す

## 完了条件

- 実機環境で `pixel_format` 指定・stop→start・複数フォーマットのキャプチャテストが通る
- `VIDEO_DEVICE_DEVICE_TEST` 未設定時に未実行であることが CI ログから判別できる
- 装置不要のバリデーションテストが通常の CI で実行される
- `frame_math` の境界値テストが追加されている
- `cargo test --workspace --all-targets` が通る
- モック・スタブを使用していない

## 解決方法

{完了時に記入}
