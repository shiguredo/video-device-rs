# examples の過剰な防御・重複・意味の不揃いを整理する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/refactor-examples-cleanup
- Polished: 2026-11-06

## 目的

examples に、サンプルとしての読みやすさを損なう実装と、example 間の不整合がある。

1. `examples/camera_preview.rs` の `strip_stride` は 45 行の関数で、防御処理と `Cow` によるゼロコピー最適化を含む。呼び出し先の raw-player の `enqueue_video_*` が結局 `to_vec()` でコピーするため、`Cow` によるゼロコピーの効果はほぼ無い。エラー時の早期 return が 9 箇所あり、サンプルとして読みにくい
2. 同ファイルに「1 回だけログを出す」ための `Once` が 5 箇所あり、うち 2 つは同じ `WARN` という名前で別スコープに定義されている
3. 同ファイルは `poll_events` の `Err` を無音で break しており、エラーが表示されない
4. `examples/device_info.rs` は同じデバイスの `formats()` を集計ループと JSON 出力ループで 2 回呼ぶ。FFI バックエンドでは毎回 C を列挙する
5. `examples/device_list.rs` は `device.format_count()` を、`examples/device_info.rs` は `formats.len()` を同じ JSON フィールド名 `format_count` で出力する。FFI バックエンドでは前者が C 報告値、後者が取得できた件数であり、意味が異なる（Media Foundation では `format_count()` も `formats.len()` を返すため一致する）
6. `examples/device_info.rs` の `pixel_format_counts` は `PixelFormat::name()` を JSON のキーにするため、異なる `Unknown(u32)` がすべて同じ "Unknown" という名前で出力され、JSON オブジェクト内でキーが重複する（件数は raw 値ごとに分かれたまま）。また `formats[].pixel_format` も `name()` を使うため、`Unknown` の raw 値が失われる

## 優先度根拠

- Low。サンプルの品質問題であり、ライブラリの挙動には影響しない
- サンプルは利用者が最初に読むコードであり、可読性の価値が高い
- `/review-code` の改善指摘として確認済み

## 現状

- `strip_stride` は `row_bytes == 0` / `height == 0` / `stride == 0` / `stride == row_bytes` / スライス長の不足 / 最終行計算 / 行ごとのコピーで分岐する
- `device_info.rs` の `for device in &device_list { let formats = device.formats(); ... }` が 2 箇所にある

## 設計方針

- `strip_stride` は単純な行コピーに統一するか、raw-player 2026.3 の `enqueue_video_nv12_strided` / `enqueue_video_i420_strided` / `enqueue_video_yuy2_strided` に任せる
- `Once` のログ抑止を共通ヘルパ 1 つに集約し、同じ `WARN` という名前の重複を解消する（例: `WARN_MJPEG` / `WARN_UNKNOWN` に改名するか、1 つの `WARN_UNSUPPORTED` に統合する）
- `poll_events` のエラーを `eprintln!` で表示する
- `formats()` は 1 回取得して使い回す
- `device_info` の `format_count` は `device.format_count()` を使い、取得件数が必要なら別フィールド名にする。`statistics.total_format_count` は取得件数の合計なので、`total_retrieved_format_count` に改名するか、`device.format_count()` の合計に変更する
- `pixel_format_counts` は `Unknown` を raw 値ごとに分ける場合は JSON キーに `PixelFormat` の `Display`（例: `Unknown(0x12345678)`）を使って一意にする。集約する場合は全 `Unknown` の件数を合計して 1 つの "Unknown" にし、その旨をコメントする。`formats[].pixel_format` の表記も選んだ方針に揃える

## 完了条件

- 上記 1〜6 が整理されている
- `cargo fmt --all --check` が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo run --example device_list` / `device_info` / `camera_preview -- --list-devices` が動作する
- JSON 出力の `format_count` の意味が example 間で一致し、取得件数を別フィールドで出力する場合は名前で区別できる

## 解決方法

{完了時に記入}
