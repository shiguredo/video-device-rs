# C バックエンド間の重複したデバイス構造体・解放処理・アクセサを統合する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/refactor-c-backend-duplication
- Polished: 2026-11-06

## 目的

`src/video_avf.m` / `src/video_v4l2.c` / `src/video_pipewire.c` に、同一の `struct VideoDevice` 定義、同一の `free_devices`、同一の 4 つのアクセサ（`device_name` / `device_unique_id` / `device_format_count` / `device_get_format`）が三重に実装されている。変更時に 3 箇所を同期する必要があり、修正漏れの温床になる。デバイス情報の共通処理を 1 箇所に集約し、各バックエンドは列挙処理とセッション処理に専念できるようにする。

## 優先度根拠

- Low。挙動に影響しない整理
- `/review-code` の改善指摘として確認済み（本審でも実ファイルで重複を再確認した）

## 現状

- `struct VideoDevice` は 3 ファイルで同じレイアウト（`name` / `unique_id` / `formats` / `format_count`）
- `*_free_devices` は 3 ファイルで同一実装
- 4 つのアクセサも 3 ファイルで同一実装
- デバイス情報の列挙処理のみがバックエンド固有（セッション処理は各バックエンド固有）
- `src/video.h` には `struct VideoDevice` の前方宣言だけがあり、実体は各バックエンドにある
- `src/device_ffi.rs` の `DeviceOps` はバックエンドごとの `video_*_device_name` / `video_*_device_unique_id` / `video_*_device_format_count` / `video_*_device_get_format` / `video_*_free_devices` を関数ポインタとして保持している
- `build.rs` は `video_avf.m` / `video_v4l2.c` / `video_pipewire.c` を個別にコンパイルしており、共通 C 実装をビルドする仕組みがない

## 設計方針

- `src/video.h` の `struct VideoDevice` 前方宣言を削除し、`struct VideoFormatEntry` の定義の後ろへ `struct VideoDevice` の完全な定義を置き、共通関数 `video_device_name` / `video_device_unique_id` / `video_device_format_count` / `video_device_get_format` / `video_device_free_devices` の宣言を追加する
- 共通実装を新規 `src/video_device.c` にまとめる
- `src/video_avf.m` / `src/video_v4l2.c` / `src/video_pipewire.c` から `struct VideoDevice` 定義と共通関数の実装を削除し、列挙処理とセッション処理などバックエンド固有の処理だけを残す
- `src/video_avf.h` / `src/video_v4l2.h` / `src/video_pipewire.h` から共通アクセサ・解放関数の宣言を削除する
- `src/device_ffi.rs` の `DeviceOps` は、`free_devices` と 4 つのアクセサを共通関数 `ffi::video_device_*` に差し替える。`enumerate_devices` だけをバックエンド固有の関数ポインタとして残す
- `build.rs` の `build_macos` / `build_linux_v4l2` / `build_linux_pipewire` で、それぞれ `src/video_device.c` を `cc::Build` の `.file()` に追加する。同一の C ファイルが複数の静的ライブラリに含まれるが、静的リンクでは必要時に 1 つだけ取り込まれるため重複シンボルにならない
- `build.rs` の `rerun-if-changed` に `src/video_device.c` を追加する
- `src/video_device.c` は純粋な C なので `-fobjc-arc` を付ける必要はない。`build_macos` は `.m` と `.c` を同じ `cc::Build` に含めるが、`clang` は C ファイルへの `-fobjc-arc` も受け付けるため、追加のビルド分離は不要（実測: `clang -fobjc-arc -c foo.c` が成功）
- Windows には C バックエンドがないため、`src/video_device.c` のビルド対象外とする

## 変更対象

- `src/video.h`: `struct VideoDevice` の前方宣言削除、完全な定義と共通関数宣言の追加
- `src/video_device.c`: 新規。共通関数の実装
- `src/video_avf.m`: `struct VideoDevice` 定義と `video_avf_free_devices` / 4 アクセサの削除
- `src/video_v4l2.c`: `struct VideoDevice` 定義と `video_v4l2_free_devices` / 4 アクセサの削除
- `src/video_pipewire.c`: `struct VideoDevice` 定義と `video_pipewire_free_devices` / 4 アクセサの削除
- `src/video_avf.h` / `src/video_v4l2.h` / `src/video_pipewire.h`: 共通関数宣言の削除
- `src/device_ffi.rs`: `DeviceOps` の関数ポインタを共通関数へ差し替え
- `build.rs`: `src/video_device.c` のビルド追加と `rerun-if-changed` 追加
- `CHANGES.md`: `## develop` の `### misc` に `[UPDATE]` エントリを追加

## 完了条件

- `struct VideoDevice` と共通アクセサ・解放処理が `src/video.h` / `src/video_device.c` の 1 箇所に集約されている
- 各バックエンドに `struct VideoDevice` の定義と共通関数の実装が残っていない
- `src/device_ffi.rs` の `DeviceOps` が共通関数を参照し、バックエンド固有の関数ポインタは `enumerate_devices` だけになっている
- macOS / Linux / Windows のビルドが通る
- `cargo fmt --all --check`、`cargo check --workspace`、`cargo test --workspace`、`cargo clippy --workspace --all-targets -- -D warnings` が通る
- 列挙・フォーマット取得の挙動が変わらない
- CI の Device Test ジョブのうち、Windows / Linux は実機キャプチャテストが通り、macOS は TCC の制約でキャプチャテストを実行しないため `device_list` による列挙確認のみ通ることを確認する
- `CHANGES.md` の `## develop` の `### misc` に `[UPDATE]` エントリが追加されている

## スコープ外

以下は `src/video_avf.m` / `src/video_pipewire.c` の改善候補だが、本 issue の目的（デバイス構造体・解放処理・アクセサの共通化）とは別のため対象外とする。同一カテゴリ・同一ファイルの整理であり、独立した issue を起票するほどの規模ではないと判断した。

- `src/video_avf.m` の `captureOutput` の形式判定 if/else と `convert_pixel_format` の対応表の二重管理
- `src/video_pipewire.c` の `video_pipewire_session_start` の 5 つのエラーパスで繰り返される後始末
- C のローカル変数命名の不統一（AVF は Objective-C の慣習に従い camelCase、V4L2 / PipeWire は snake_case）
- `video_avf_session_create` / `video_pipewire_session_start` / `on_process` の肥大化
- `/dev/video` の上限 64、`VIDIOC_REQBUFS` の 4、`params_buffer` の 1024 などのマジックナンバーと、`-1` / `-2` / `-3` の負の戻り値コードの根拠コメント

## 解決方法

{完了時に記入}
