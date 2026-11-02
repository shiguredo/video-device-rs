# types.rs から Media Foundation 内部ヘルパを分離し、error と types の依存方向を整理する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/refactor-types-mf-separation
- Polished: {YYYY-MM-DD}

## 目的

`src/types.rs` は公開型（`PixelFormat` / `VideoFormat` / `VideoCaptureConfig` / `VideoFrame` / `VideoFrameOwned` / `PixelBuffer`）と、Media Foundation 専用の内部ヘルパ（`CoInitGuard` / `CoTaskMemActivateArrayGuard` / `guid_to_pixel_format` / `pixel_format_to_guid`）を同一ファイルに持っており、420 行に肥大化している。公開 API の型定義を読むときに Windows 内部実装が混在する。

また、`src/error.rs` が `crate::types::PixelFormat` に依存し、`src/types.rs` が `crate::Error` / `crate::Result` に依存しており、モジュール間の依存方向が循環している（コンパイルは通る）。MF ヘルパを分離すれば `types` から `Error` への依存を外せる。

## 優先度根拠

- Low。挙動に影響しない構造の整理
- 公開型のファイルとして読みにくく、内部実装の変更が公開 API のファイルに波及する
- `/review-code` の改善指摘として確認済み

## 現状

- `src/types.rs` の後半に `#[cfg(enable_mf)]` の `CoInitGuard` / `CoTaskMemActivateArrayGuard` / GUID 変換関数がある
- `src/error.rs` は `use crate::types::PixelFormat;`
- `src/types.rs` は `crate::Error` / `crate::Result` を MF ヘルパ内で使用

## 設計方針

- MF 専用ヘルパを `src/mf.rs` などの内部モジュールへ移す
- `types` から `Error` への依存を外し、依存方向を `error → types` の一方向にする
- 公開 API に影響する場合は CHANGES.md の記載方針を確認する（内部モジュールの移動のみなら記載不要）

## 完了条件

- `src/types.rs` が公開型とバックエンド共通ヘルパのみになる
- `types` が `error` に依存しない
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- 全バックエンドのビルドとテストが通る

## 解決方法

{完了時に記入}
