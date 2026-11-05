# 実機キャプチャ・エラー経路・装置不要バリデーションのテストを拡充する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/add-test-strategy-expansion
- Polished: 2026-11-06

## 目的

テスト戦略に次の穴がある。

1. `tests/test_capture.rs` のキャプチャテストは既定 `VideoCaptureConfig` での start → 5 フレーム受信 → stop のみを検証する。`pixel_format` を明示したキャプチャ、I420 / YUY2 / MJPEG、`stop()` 後の再 `start()`、二重 `stop()`、存在しない `device_id`、複数デバイス、start しないままの drop を検証していない。受信フレームも width / height / stride の正値と data 非空のみで、`data.len()` と stride × 高さの整合を見ていない。
2. `pixel_format` を明示しないため、`frame_callback` の YUY2 / I420 分岐と `frame_math` の該当関数が実機で実行される保証がない。デフォルト選択はバックエンド依存であり、V4L2 は NV12 失敗時に YUY2、PipeWire / Windows は YUY2 / I420 も候補になるが、CI のデバイスがどのフォーマットを選ぶかは固定されていない。フォーマット指定の欠落は、C と Rust のフレーム長契約の不一致を狙って検出できないことを意味する。
3. macOS の実キャプチャは CI で明示的にスキップされ、`VIDEO_DEVICE_DEVICE_TEST` 未設定時はテストが早期 return して「ok」と表示される。スキップメッセージは `eprintln!` で出しているが、テストハーネスが成功したテストの出力を捕捉するため、既定の `cargo test` 出力には現れない。未実行であることが CI の緑から判別できない。
4. ハードウェア不要のバリデーションが未テスト。`PixelFormat::Unknown` の拒否と NUL バイトを含む `device_id` の拒否は FFI バックエンド (AVF / V4L2 / PipeWire) の `FfiCaptureImpl::new` で、デバイスを開く前に検証できる。AVF / PipeWire での `PixelFormat::Mjpeg` 拒否も `new_avf` / `new_pipewire` でデバイスを開く前に検証できる。Windows の `validate_capture_config_for_windows` は `VideoCapture::new_mf` から到達でき、COM 初期化より前に `InvalidCaptureConfig` を返す。ただし Windows の `PixelFormat::Unknown` は `activate_device` の後で拒否されるため、通常の CI では装置不要の検証対象にしない。
5. `frame_math` の大きな値の境界 (`i32::MAX` 近傍) が未テスト。64 bit 環境では `i32` の正値同士の積は `usize` に収まるため、`checked_mul` が `None` になるのは `yuy2_packed_frame_bytes_win` の `width.checked_mul(2)` が `i32` の範囲を超える場合だけである。32 bit 環境では `usize` の上限を超える場合に `None` になる。
6. `tests/test_pipewire.rs` は `format_count == formats.len()` を assert するが、公開 doc (`src/device.rs` の `format_count` / `formats`) は一致を保証していない。`unique_id()` のエラーを `unwrap_or_default()` で握りつぶしている。`pipewire_multiple_devices_formats` は実際には unique_id の重複を検証しており名前と内容が一致しない。`pipewire_format_index_bounds` も名前と内容が一致しない。`formats` が空の Video/Source ノードを許容しない。
7. `tests/test_types.rs` の `capture_config_accepts_an_explicit_pixel_format` は構造体を作って同じ値を assert するだけで実装の写経。`frame_math` の `mjpeg_payload_bytes_returns_input` は実質的に恒等関数の確認のみで、境界値以外の性質を検証していない。
8. PBT の `SEED_ENV` / `CASES` / PixelFormat サンプラーが `prop_types.rs` と `prop_error.rs` で重複している。

なお、frame_math の PBT 追加は既存の PBT カバレッジ拡充 issue が追跡しているため、本 issue では単体テストの境界値のみを扱う。同 issue の「`pbt/tests/prop_types.rs` は変更しない」は `PixelFormat::Unknown` の対応に関する条件であり、本 issue の共通ヘルパー化による変更は対象外とする。

## 優先度根拠

- Medium。C / Rust 間のフレーム長契約の不一致や状態遷移のバグは、実機テストとバリデーションテストでしか検出できない。
- `/review-code` の重要指摘として確認済み。

## 現状

- `tests/test_capture.rs` の設定は `..VideoCaptureConfig::default()` で `pixel_format: None`。
- `cargo test --workspace` で device test は環境変数未設定時に早期 return する。
- `tests/test_pipewire.rs` は列挙のみを検証する。
- `tests/test_capture.rs` のスキップメッセージは `eprintln!` で出しているが、成功したテストの出力はテストハーネスに捕捉され、`cargo test` の既定出力には現れない。

## 設計方針

- 実機テストを拡張する。フォーマット指定のキャプチャは、デバイスが対応するフォーマットを `formats()` から引き、`VideoFormat` の width / height と `min_fps` / `max_fps` の範囲内の fps を設定する。指定フォーマットが `formats()` にない場合はスキップし、`VideoCapture::new*` が `SessionCreateFailed` / `UnsupportedPixelFormat` を返した場合もそのフォーマットをスキップする。1 つもフォーマットでフレームを受信できなかった場合はテストを失敗させる。PipeWire は指定と異なるフォーマットに交渉されうるため、受信フレームの `pixel_format` を記録し、指定フォーマットが返らない場合は実際に返ったフォーマットの分岐を検証する。MJPEG は `enable_mjpeg` 時のみ対象とする。
- start → stop → start → フレーム再受信、二重 stop、存在しない device_id、複数デバイス、start しないままの drop を検証する。存在しない device_id はバックエンドごとの既知の挙動（macOS は既定デバイスへフォールバックし、既定デバイスが無い場合は `SessionCreateFailed`、V4L2 は `SessionCreateFailed`、Windows は `DeviceNotFound`、PipeWire は `Ok` ならフレーム受信、`Err` なら `SessionStartFailed` または `SessionCreateFailed` を許容）を `#[cfg]` で分けて検証する。複数デバイスは 2 台以上ある場合に `formats()` が空でない各デバイスから 1 フレーム受信し、1 台の場合と `formats()` が空のデバイスはスキップする。二重 stop は 2 回目が no-op であること、start しないままの drop は panic しないことを確認する。
- 受信フレームの検証はフォーマットごとに行う。NV12: `data.len() == stride * height`、`uv_data.len() == stride_uv * ceil(height / 2)`。I420: `data.len() == stride * height`、`uv_data.len() == stride_uv * ceil(height / 2) * 2`。YUY2: `data.len() == stride * height`、`uv_data` は None。MJPEG: `stride == 0`、`uv_data` は None、`data.len() > 0`。MJPEG では `data.len() == stride * height` を要求しない。
- 装置不要のバリデーションは `tests/test_capture.rs` に feature ごとの `#[cfg]` を付けた integration test として追加する。`PixelFormat::Unknown` と NUL バイト `device_id` は FFI バックエンドに限定し、Windows は `new_mf` の `InvalidCaptureConfig` を検証する。AVF / PipeWire の `PixelFormat::Mjpeg` 拒否（および V4L2 で `mjpeg` feature 無効時の拒否）も `#[cfg]` で検証する。`tests/test_capture.rs` のファイル doc を「実機テストと装置不要テストを含む」に更新する。
- `frame_math` の境界値テストを `src/frame_math.rs` の `#[cfg(test)]` に追加する。64 bit では `yuy2_packed_frame_bytes_win` の `width > i32::MAX / 2` だけが `None` になり、他の関数は `i32::MAX` 近傍でも `Some` を返す。32 bit では `usize` の上限を超える場合に `None` を期待する。`i32::MAX` 級の積で必ず `None` を期待しない。
- `tests/test_pipewire.rs` の `format_count == formats.len()` を `format_count >= formats.len()` に変更する。`pipewire_multiple_devices_formats` を `pipewire_unique_ids_are_unique` などに改名し、`unique_id()` は `expect` でエラーを表面化する。`formats` が空のデバイスを許容し、少なくとも 1 台のデバイスに formats があることを確認する。`pipewire_format_index_bounds` も内容に合う名前にする。
- 弱いテストを削除または意味のある検証に置き換える。`capture_config_accepts_an_explicit_pixel_format` は削除する（実機テストのフォーマット指定ケースで同じ設定を扱う）。`mjpeg_payload_bytes_returns_input` は `mjpeg_rejects_non_positive_payload_size` と統合するか、境界値テストに置き換える。
- PBT の共通定数・サンプラーを `pbt/tests/helpers/` に集約する。`SEED_ENV` / `CASES` / PixelFormat サンプラーを共通モジュールに移し、`prop_types.rs` と `prop_error.rs` から `#[path]` で参照する。`mod.rs` は使わない。
- macOS の実キャプチャを実行できない場合の手動検証手順を README の macOS 節または `docs/MACOS.md` に残す。CI では実行されないこと、`VIDEO_DEVICE_DEVICE_TEST=1 cargo test --test test_capture` の実行、TCC のカメラ権限確認を含める。
- CI の Device Test ジョブで `VIDEO_DEVICE_DEVICE_TEST` 未設定時のスキップがログに残るようにする。`cargo test --workspace -- --nocapture` を実行する、またはスキップ件数を出力するステップを追加する。macOS の device test ステップがスキップされることもログから分かるようにする。MJPEG の実機テストは `--features mjpeg` を有効にした Device Test ステップで実行し、デバイスが MJPEG に対応しない場合はテスト内でスキップする。

## 変更対象

- `tests/test_capture.rs`
- `tests/test_pipewire.rs`
- `tests/test_types.rs`
- `src/frame_math.rs` (`#[cfg(test)]` のみ)
- `pbt/tests/helpers/` (新規)
- `pbt/tests/prop_types.rs`
- `pbt/tests/prop_error.rs`
- `.github/workflows/ci.yml`
- `README.md` または `docs/MACOS.md`
- `CHANGES.md`

## 完了条件

- 実機環境で、デバイスが対応するフォーマット（NV12 / YUY2 / I420、`mjpeg` feature 有効時は MJPEG）のうち 1 つ以上について、`pixel_format` を明示したキャプチャでフレームを受信できる。`VideoCapture::new*` が成功したフォーマットが複数ある場合は複数フォーマットを検証する。macOS は CI で実行できないため、手動検証手順に従って確認する。
- start → stop → start → フレーム再受信、二重 stop、存在しない device_id、複数デバイス（2 台以上の場合）、start しないままの drop のテストが追加され、バックエンドごとの期待値に従って通る。
- 受信フレームの長さ検証がフォーマットごとの式（NV12 / I420 / YUY2 / MJPEG）で行われ、MJPEG で `stride == 0` を許容する。
- `VIDEO_DEVICE_DEVICE_TEST` 未設定時に未実行であることが CI ログから判別できる（`-- --nocapture` またはスキップ件数の出力）。
- CI の Device Test ジョブに `--features mjpeg` の実機テストステップが追加され、デバイスが MJPEG に対応する場合に MJPEG のキャプチャが検証される。
- 装置不要のバリデーションテストが通常の CI で実行される。`PixelFormat::Unknown` と NUL `device_id` は FFI バックエンド、Windows は `InvalidCaptureConfig`、AVF / PipeWire は `PixelFormat::Mjpeg` の拒否を対象とする。V4L2 で `mjpeg` feature 無効時の `PixelFormat::Mjpeg` 拒否も対象とする。
- `frame_math` の `i32::MAX` 近傍の境界値テストが追加されている。64 bit では `yuy2_packed_frame_bytes_win` の `width > i32::MAX / 2` が `None`、他の関数は `Some`。32 bit では `usize` の上限を超える場合に `None`。
- `tests/test_pipewire.rs` の assert が公開契約 (`format_count >= formats.len()`) に一致し、命名と `unique_id()` のエラー処理が修正されている。空の `formats` を許容する。
- 弱いテストが削除または意味のある検証に置き換えられている。
- PBT の共通定数・サンプラーが `pbt/tests/helpers/` に集約されている。
- macOS の手動検証手順が README または docs に記載されている。
- `cargo test --workspace --all-targets` が通る。
- モック・スタブを使用していない。
- `CHANGES.md` の `## develop` の `### misc` にテスト拡充のエントリを追加する。

## 解決方法

{完了時に記入}
