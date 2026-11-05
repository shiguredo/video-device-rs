# README / docs / examples の重複と、コードコメントの実装不一致を修正する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-docs-comments-consistency
- Polished: 2026-11-06

## 目的

ドキュメントとコードコメントに、重複・誤記・実装との不一致がある。

1. README と docs/LINUX.md で Linux のビルド依存（V4L2 の `libclang-dev`、PipeWire の `libclang-dev` / `libpipewire-0.3-dev` / `pkg-config`）、ビルド・実行コマンド、MJPEG の手順が重複している。片方だけ更新されると食い違う
2. `src/lib.rs` のコールバック内 `stop()` の禁止が Windows 限定に見える。V4L2 は `pthread_join` の失敗で停止保証が破れ、PipeWire は再帰ロックと thread loop の自己 join で停止保証が破れ、AVF は同じ serial queue への `dispatch_sync` でデッドロックする。全バックエンドで禁止すべき
3. examples/README.md は README のサンプル節と内容が重複している
4. `src/frame_math.rs` の doc とテストコメントが存在しないファイル `video_c.m` を参照している（実体は `src/video_avf.m`）
5. `src/video_pipewire.c` のコメントが `nv12_plane_sizes` / `i420_plane_sizes` の参照先を `capture.rs` としている（実体は `src/frame_math.rs`）
6. `src/video.h` の FourCC コメントが誤り。`0x3231564E` は ASCII FourCC "NV12" で macOS の '420v'（`0x34323076`）ではなく、`0x32595559` は "YUY2" で 'yuvs'（`0x79757673`）ではない。AVF は CVPixelBuffer の型を独自定数へ変換している
7. `src/types.rs` の `PixelFormat` の doc が `from_raw` を公開 API のように書いているが、実体は `pub(crate)` であり、利用者は呼べない
8. `src/capture_ffi.rs` / `src/capture_mf.rs` の内部 impl の doc が公開 API の `start` / `stop` 契約を丸写ししており、片方だけ更新されると内部 doc だけ古くなる
9. `Cargo.toml` の build-dependencies のコメントが実際の用途と一致していない。`bindgen` は C/C++ ヘッダ、`cc` は C/C++ コード、`pkg-config` は V4L2 等を検出するという記述になっているが、実際は `bindgen` は C ヘッダ、`cc` は C と Objective-C、`pkg-config` は `libpipewire-0.3` のみを対象にしている
10. `src/types.rs` の `VideoCaptureConfig` の doc は「Linux（PipeWire 等）では不正値を C 側が既定解像度・フレームレートに置き換える場合がある」としているが、実際に置き換えるのは PipeWire のみ。V4L2 と AVF は不正値をそのまま C に渡す
11. `src/device.rs` の `format_count` / `formats` の doc は「C 側が報告するエントリ数」「NULL で取得できなかったインデックスをスキップする」という FFI 専用の説明になっており、Media Foundation 実装には当てはまらない
12. `src/types.rs` の FourCC 定数群のグループ見出しが先頭の定数への `///` になっており、`--document-private-items` では「NV12 の説明」として扱われる。グループ見出しは `//` にし、各定数の説明が必要なら個別に `///` を付けるべき

## 優先度根拠

- Low。挙動には影響しないが、利用者向けドキュメントの誤りと重複は保守コストと誤用を招く
- `/review-code` の指摘として確認済み

## 現状

- README の「特徴」からバックエンド一覧は削除され、「対応プラットフォーム」の 1 表に整理済み。ただし README と docs/LINUX.md の Linux ビルド依存・ビルド・実行コマンド・MJPEG 手順は重複したまま
- README の注意書きは「コールバック内から `stop()` を呼んだり `VideoCapture` を drop したりしないでください」と全バックエンド禁止に更新済み。`src/lib.rs` は「特に Windows ではキャプチャスレッドが `join` 自身しデッドロックしうるためです」のままで、Windows 限定に見える
- README のサンプル節と examples/README.md は、デバイス一覧・デバイス情報・カメラプレビューの説明、コマンド、オプション表が重複したまま
- README と examples/README.md には `--video-device-id` / `--duration` が既に記載済み
- `src/frame_math.rs` の I420 の doc コメントとテストコメントに `macOS video_c.m` の記述がある
- `src/video_pipewire.c` の `required_bytes_nv12` / `required_bytes_i420` のコメントが `nv12_plane_sizes` / `i420_plane_sizes` の参照先を `capture.rs` としている
- `src/video.h` のコメントは `// 値は macOS の CVPixelBuffer が用いる FourCC ('420v' 等) に対応する` で、実際の値は ASCII FourCC の "NV12" / "YUY2" / "I420" / "MJPG"
- `src/types.rs` の `PixelFormat` の doc は `from_raw` を公開 API のように書いているが、実体は `pub(crate)` であり公開 rustdoc には表示されない
- `src/capture_ffi.rs` / `src/capture_mf.rs` の内部 impl の doc は公開 API の `start` / `stop` 契約を丸写ししている
- `Cargo.toml` の build-dependencies のコメントは、`bindgen` が C/C++ ヘッダ、`cc` が C/C++ コード、`pkg-config` が V4L2 等を検出するという記述になっている
- `src/types.rs` の `VideoCaptureConfig` の doc は「Linux（PipeWire 等）では不正値を C 側が既定解像度・フレームレートに置き換える場合がある」としている
- `src/device.rs` の `format_count` / `formats` の doc は「C 側が報告するエントリ数」「NULL で取得できなかったインデックスをスキップする」という FFI 専用の説明になっている
- `src/types.rs` の FourCC 定数群のグループ見出しが先頭の定数への `///` になっており、`--document-private-items` では「NV12 の説明」として扱われる

## 設計方針

- README と docs/LINUX.md の重複は、Linux 固有の詳細（ビルド依存・ビルド・実行コマンド・MJPEG の手順）を docs/LINUX.md に集約し、README からは docs/LINUX.md を参照する形にする
- examples の説明は examples/README.md に寄せ、README からは examples/README.md を参照する形にする
- `src/lib.rs` のコールバック制約を、全バックエンドで `stop()` と drop を禁止する記述に書き直す。各バックエンドで停止保証が破れる理由（V4L2 の `pthread_join` 失敗、PipeWire の再帰ロックと thread loop の自己 join、AVF の `dispatch_sync`）も簡潔に残す
- `src/frame_math.rs` / `src/video_pipewire.c` / `src/video.h` の誤ったファイル名・FourCC のコメントを実装に合わせる
- `src/types.rs` の `PixelFormat` の doc は、`from_raw` が `pub(crate)` の内部メソッドであることを明記するか、利用者向けの説明から削除する
- `src/capture_ffi.rs` / `src/capture_mf.rs` の内部 impl の doc は、公開 API の `VideoCapture::start` / `stop` の契約を参照する 1 行に置き換える
- `Cargo.toml` の build-dependencies のコメントを実際の用途（`bindgen` は C ヘッダ、`cc` は C と Objective-C、`pkg-config` は `libpipewire-0.3`）に合わせる
- `src/types.rs` の `VideoCaptureConfig` の doc は、既定値への置き換えが PipeWire のみであることを明記する。V4L2 と AVF は値をそのまま C に渡すことも書く
- `src/device.rs` の `format_count` / `formats` の doc は、FFI 固有の NULL スキップ説明を FFI 実装側に移し、公開 API ではバックエンド共通の契約を書く。MF では `format_count` と `formats` の要素数が一致することを明記する
- `src/types.rs` の FourCC 定数群のグループ見出しを `//` にし、各定数の説明が必要なら個別に `///` を付ける
- `src/video.h` の FourCC コメントは 0067 の完了条件、`src/capture_ffi.rs` / `src/capture_mf.rs` の内部 impl の doc は 0068 の項目 5 と重複する。先にそちらが実施された場合は、本 issue では該当箇所を変更しない

## 完了条件

- 目的に列挙した README / docs / examples の重複が解消され、README から docs/LINUX.md と examples/README.md を参照している
- `--video-device-id` / `--duration` が README と examples/README に記載されている
- `src/lib.rs` のコールバック内 `stop()` / drop の制約が全バックエンドについて正しく記載されている
- `src/frame_math.rs` / `src/video_pipewire.c` / `src/video.h` のファイル名・FourCC のコメントが実装と一致している
- `src/types.rs` の `PixelFormat` / `VideoCaptureConfig` の doc と FourCC 定数群のグループ見出しが実装・規約と一致している
- `src/device.rs` の `format_count` / `formats` の doc が FFI と MF の両方の実装と一致している
- `src/capture_ffi.rs` / `src/capture_mf.rs` の内部 impl の doc が公開 API の契約を参照する形になっている
- `Cargo.toml` の build-dependencies のコメントが実際の用途と一致している
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` が通る
- markdown の表示に破綻が無い

## 解決方法

{完了時に記入}
