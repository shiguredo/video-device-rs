# README / docs / examples の重複と、コードコメントの実装不一致を修正する

- Priority: Low
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-docs-comments-consistency
- Polished: {YYYY-MM-DD}

## 目的

ドキュメントとコードコメントに、重複・誤記・実装との不一致がある。

1. README の「特徴」と「対応プラットフォーム」でバックエンド一覧がほぼ同文で重複している
2. README と docs/LINUX.md で PipeWire のビルド依存と MJPEG の手順が重複している。片方だけ更新されると食い違う
3. README / `src/lib.rs` の「特に Windows ではデッドロックしうる」は不正確。V4L2 は `pthread_join` の失敗で停止保証が破れ、PipeWire は再帰ロックと thread loop の自己 join で破綻し、AVF は同じ serial queue への `dispatch_sync` でデッドロックする。全バックエンドで禁止すべき
4. README の「デッドロックしえます」は「デッドロックしうる」等に統一すべき（`src/lib.rs` と表記が不統一）
5. README / examples/README.md に `camera_preview` の `--video-device-id` と `--duration` の記載が無い
6. examples/README.md は README のサンプル節と内容が重複している
7. `src/frame_math.rs` の doc が存在しないファイル `video_c.m` を参照している（実体は `src/video_avf.m`）
8. `src/video_pipewire.c` のコメントが `nv12_plane_sizes` / `i420_plane_sizes` の参照先を `capture.rs` としている（実体は `src/frame_math.rs`）
9. `src/video.h` の FourCC コメントが誤り。`0x3231564E` は ASCII の "NV12" で macOS の '420v'（`0x34323076`）ではなく、`0x32595559` は "YUY2" で 'yuvs'（`0x73767579`）ではない。AVF は CVPixelBuffer の型を独自定数へ変換している
10. `src/types.rs` の `PixelFormat` の doc が `from_raw` を公開 API のように書いているが、実体は `pub(crate)` であり、利用者は呼べない
11. `src/capture_ffi.rs` / `src/capture_mf.rs` の内部 impl の doc が公開 API の `start` / `stop` 契約を丸写ししており、片方だけ更新されると内部 doc だけ古くなる
12. `Cargo.toml` の `pkg-config` のコメントは「Linux などで pkg-config を介してシステムライブラリ (V4L2 等) を検出する」としているが、pkg-config を使うのは `libpipewire-0.3` のみ。`cc` のコメントは「C/C++ コードをコンパイル」としているが、対象は C と Objective-C のみ
13. `src/types.rs` の `VideoCaptureConfig` の doc は「Linux（PipeWire 等）では不正値を C 側が既定値に置き換える」としているが、実際に置き換えるのは PipeWire のみ。V4L2 と AVF は不正値をそのまま C に渡す
14. `src/device.rs` の `format_count` / `formats` の doc は「C 側が報告するエントリ数」「NULL で取得できなかったインデックスをスキップする」という FFI 専用の説明になっており、Media Foundation 実装には当てはまらない
15. `src/types.rs` の FourCC 定数群の `///` が先頭の定数にだけ付いており、rustdoc 上は「NV12 の説明」になる。グループ見出しは `//` にすべき

## 優先度根拠

- Low。挙動には影響しないが、利用者向けドキュメントの誤りと重複は保守コストと誤用を招く
- `/review-code` の指摘として確認済み

## 現状

- README の「特徴」「対応プラットフォーム」で同じ 3 行が重複
- README の注意書きは `キャプチャのコールバック内から stop() を呼ばないでください。特に Windows ではデッドロックしえます。`
- `src/frame_math.rs` の I420 の doc コメントに `macOS video_c.m` の記述がある
- `src/video.h` のコメントは `// 値は macOS の CVPixelBuffer が用いる FourCC ('420v' 等) に対応する`

## 設計方針

- README は概要と 1 つのバックエンド表に集約し、詳細は docs/LINUX.md へリンクする
- examples の説明は examples/README.md に寄せ、README からは参照する形にする
- コールバック内 `stop()` / drop の禁止を全バックエンドの制約として書き直す
- 誤ったファイル名・FourCC のコメントを実装に合わせる
- 内部 impl の doc は「公開 API の契約を参照」の 1 行に置き換える

## 完了条件

- README / docs / examples の内容重複が解消されている
- `--video-device-id` / `--duration` が README と examples/README に記載されている
- コールバック内 `stop()` / drop の制約が全バックエンドについて正しく記載されている
- コードコメントのファイル名・FourCC・公開範囲が実装と一致している
- `cargo doc` と markdown の表示に破綻が無い

## 解決方法

{完了時に記入}
