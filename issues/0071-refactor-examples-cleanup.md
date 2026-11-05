# examples の過剰な防御・重複・意味の不揃いを整理する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/refactor-examples-cleanup
- Polished: {YYYY-MM-DD}

## 目的

examples に、サンプルとしての読みやすさを損なう実装と、example 間の不整合がある。

1. `examples/camera_preview.rs` の `strip_stride` は 45 行の防御処理と `Cow` によるゼロコピー最適化を含む。呼び出し先で結局コピーされるため最適化の効果はほぼ無く、早期 return が 6 箇所あってサンプルとして読みにくい
2. 同ファイルに「1 回だけログを出す」ための `Once` が 5 箇所あり、うち 2 つは同じ `WARN` という名前で別スコープに定義されている
3. 同ファイルは `poll_events` の `Err` を無音で break しており、エラーが表示されない
4. `examples/device_info.rs` は同じデバイスの `formats()` を集計ループと JSON 出力ループで 2 回呼ぶ。FFI バックエンドでは毎回 C を列挙する
5. `examples/device_list.rs` は `device.format_count()`（C 報告値）、`examples/device_info.rs` は `formats.len()`（取得できた件数）を同じ JSON フィールド名 `format_count` で出力する。両者の意味が異なる
6. `examples/device_info.rs` の `pixel_format_counts` は `PixelFormat::name()` をキーにするため、異なる `Unknown(u32)` が 1 つの "Unknown" に集約される

## 優先度根拠

- Low。サンプルの品質問題であり、ライブラリの挙動には影響しない
- サンプルは利用者が最初に読むコードであり、可読性の価値が高い
- `/review-code` の改善指摘として確認済み

## 現状

- `strip_stride` は `row_bytes == 0` / `height == 0` / `stride == 0` / `stride == row_bytes` / 最終行計算 / 行ごとのコピーで分岐する
- `device_info.rs` の `for device in &device_list { let formats = device.formats(); ... }` が 2 箇所にある

## 設計方針

- `strip_stride` は単純な行コピーに統一するか、raw-player 側の stride 対応 API に任せる
- `Once` のログ抑止を共通ヘルパ 1 つに集約する
- `poll_events` のエラーを `eprintln!` で表示する
- `formats()` は 1 回取得して使い回す
- `device_info` の `format_count` は `device.format_count()` を使い、取得件数が必要なら別フィールド名にする
- `pixel_format_counts` は `Unknown` を raw 値ごとに分けるか、集約する旨をコメントする

## 完了条件

- 上記 1〜6 が整理されている
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo run --example device_list` / `device_info` / `camera_preview -- --list-devices` が動作する
- JSON 出力のフィールドの意味が example 間で一致している

## 解決方法

{完了時に記入}
