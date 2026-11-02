# C バックエンド間の重複したデバイス構造体・解放処理・アクセサを統合する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/refactor-c-backend-duplication
- Polished: {YYYY-MM-DD}

## 目的

`src/video_avf.m` / `src/video_v4l2.c` / `src/video_pipewire.c` に、同一の `struct VideoDevice` 定義、同一の `free_devices`、同一の 4 つのアクセサ（`device_name` / `device_unique_id` / `device_format_count` / `device_get_format`）が三重に実装されている。変更時に 3 箇所を同期する必要があり、修正漏れの温床になる。

## 優先度根拠

- Low。挙動に影響しない整理
- `/review-code` の改善指摘として確認済み

## 現状

- `struct VideoDevice` は 3 ファイルで同じレイアウト（`name` / `unique_id` / `formats` / `format_count`）
- `*_free_devices` は 3 ファイルで同一実装
- 4 つのアクセサも 3 ファイルで同一実装
- バックエンド固有なのは列挙処理のみ
- `src/video_avf.m` の `captureOutput` の形式判定 if/else と `convert_pixel_format` の対応表が二重管理になっている
- `src/video_pipewire.c` の `video_pipewire_session_start` は 5 つのエラーパスでほぼ同一の後始末を繰り返している
- C のローカル変数命名が AVF は camelCase、V4L2 / PipeWire は snake_case で不統一
- `video_avf_session_create`、`video_pipewire_session_start`、`on_process` が 1 関数として肥大化している
- `/dev/video` の上限 64、`VIDIOC_REQBUFS` の 4、`params_buffer` の 1024 などのマジックナンバーと、`-1` / `-2` / `-3` の負の戻り値コードの根拠がコメントされていない

## 設計方針

- `src/video.h` に `struct VideoDevice` の定義と共通アクセサ宣言を置く
- 共通実装を `src/video_device.c` にまとめ、`build.rs` で全バックエンドのビルドに含める
- 各バックエンドは列挙処理だけを残す
- AVF は `.m` と `.c` の混在になるため、共通部分を C に置けるか確認する

## 完了条件

- `struct VideoDevice` と共通アクセサ・解放処理が 1 箇所に集約されている
- macOS / Linux / Windows のビルドが通る
- 列挙・フォーマット取得の挙動が変わらない
- 実機 device test が通る

## 解決方法

{完了時に記入}
