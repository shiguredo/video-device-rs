# types.rs から Media Foundation 内部ヘルパを分離し、error と types の依存方向を整理する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/refactor-types-mf-separation
- Polished: 2026-11-06

## 目的

`src/types.rs` は公開型（`PixelFormat` / `VideoFormat` / `VideoCaptureConfig` / `VideoFrame` / `VideoFrameOwned` / `PixelBuffer`）と、Media Foundation 専用の内部ヘルパ（`CoInitGuard` / `CoTaskMemActivateArrayGuard` / `guid_to_pixel_format` / `pixel_format_to_guid`）を同一ファイルに持っており、420 行に肥大化している。公開 API の型定義を読むときに Windows 内部実装が混在する。

また、`src/error.rs` が `crate::types::PixelFormat` に依存し、`src/types.rs` が `crate::Error` / `crate::Result` に依存しており、モジュール間の依存方向が循環している（コンパイルは通る）。MF ヘルパを分離すれば `types` から `Error` への依存を外せる。

## 優先度根拠

- Low。挙動に影響しない構造の整理
- 公開型のファイルとして読みにくく、内部実装の変更が公開 API のファイルに波及する
- `/review-code` の改善指摘として起票され、本審で `src/types.rs` / `src/error.rs` を実ファイルで照合して確認済み

## 現状

- `src/types.rs` の後半に `#[cfg(enable_mf)]` の `CoInitGuard` / `CoTaskMemActivateArrayGuard` / GUID 変換関数がある
- `src/error.rs` は `use crate::types::PixelFormat;`
- `src/types.rs` は `crate::Error` / `crate::Result` を MF ヘルパ内で使用

## 設計方針

- MF 専用ヘルパを `src/mf.rs` へ移し、`src/lib.rs` に `#[cfg(enable_mf)] mod mf;` を追加する
- `types` から `Error` への依存を外し、依存方向を `error → types` の一方向にする
- `src/types.rs` のモジュール doc から MF ヘルパに関する記述を削除し、`src/mf.rs` の先頭にモジュールの責務を 1〜2 行で書く
- 内部モジュールの移動と import の付け替えだけで公開 API は変わらないが、リファクタリングとして `CHANGES.md` の `## develop` の `### misc` に `[UPDATE]` エントリを追加する（`shiguredo-changelog` の「機能に直接影響しない変更（ドキュメント追加、リファクタリング等）は `### misc` サブセクションに記載すること」に従う）

## 変更対象

- `src/types.rs`: MF 専用ヘルパ 4 種とそれらの `#[cfg(enable_mf)]` を削除し、モジュール doc を更新する
- `src/mf.rs`（新規）: MF 専用ヘルパ 4 種を移し、モジュール doc を書く
- `src/lib.rs`: `#[cfg(enable_mf)] mod mf;` を追加する
- `src/capture_mf.rs` / `src/device_mf.rs`: MF ヘルパの import を `crate::types` から `crate::mf` に変更する
- `CHANGES.md`: `## develop` の `### misc` に `[UPDATE]` エントリを追加する
- `src/error.rs`: 変更なし（`use crate::types::PixelFormat;` は維持する）

## 関連 issue

- `issues/0050-bug-avf-uyvy-mislabel.md` は `src/types.rs` の `pixel_format_to_guid` を変更対象として挙げている
- `issues/0054-bug-windows-mf-sta-com-init.md` は `src/types.rs` の `CoInitGuard::new` と `enable_mf` の単体テストを変更対象として挙げている
- `issues/0073-add-uyvy-linux-windows.md` は `src/types.rs` の `guid_to_pixel_format` / `pixel_format_to_guid` を変更対象として挙げている
- `issues/0043-bug-callback-internal-drop-uaf.md` は `src/types.rs` の `CoInitGuard` を説明に引用している
- 本 issue を先に実施した場合は、これらの issue の参照を `src/mf.rs` に読み替える

## 完了条件

- `src/types.rs` に MF 専用ヘルパ（`CoInitGuard` / `CoTaskMemActivateArrayGuard` / `guid_to_pixel_format` / `pixel_format_to_guid`）が残っていない
- `src/types.rs` が公開型と、公開型の実装に必要な最小限のヘルパのみになる
- `src/types.rs` のモジュール doc から MF ヘルパに関する記述が削除され、`src/mf.rs` にモジュール doc がある
- `src/capture_mf.rs` / `src/device_mf.rs` の MF ヘルパの import が `crate::mf` に更新されている
- `types` が `error` に依存しない
- `cargo fmt --all --check` が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- 全バックエンドのビルドとテストが通る
- `CHANGES.md` の `## develop` の `### misc` に `[UPDATE]` エントリが追加されている

## 解決方法

{完了時に記入}
