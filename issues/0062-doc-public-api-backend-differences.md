# 公開 API のバックエンド差を doc に明記する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-public-api-doc-backend-differences
- Polished: 2026-11-05

## 目的

公開 API の契約がバックエンド（macOS AVF / Linux V4L2 / Linux PipeWire / Windows MF）で異なるのに、公開 API の rustdoc（`cargo doc` で生成される doc）がその差を説明していない。クロスプラットフォームで使う利用者が誤用する。
README と `skills/shiguredo-video-device/SKILL.md` にはバックエンド差の記載があるが、rustdoc には反映されていない。

1. `VideoCapture` の auto trait: macOS / Linux は `Send` だが、Windows は `MfCaptureImpl` が `CoInitGuard`（`!Send`）を保持するため `!Send`。`Sync` は全バックエンドで無い。rustdoc の説明文に記載が無い
2. `timestamp_us` の起点: AVF はサンプルの PTS、V4L2 はドライバのタイムスタンプ、MF はストリーム相対の 100ns、PipeWire はグラフ時刻 / `CLOCK_MONOTONIC`。rustdoc は「マイクロ秒」としか書いていない
3. `device_id` の解決規則: AVF は `uniqueID`、V4L2 は `/dev/videoN`、MF はシンボリックリンク、PipeWire は node name。`unique_id()` をそのまま渡せることが書かれていない。`None` の既定も V4L2 は `/dev/video0` 固定で、列挙結果の先頭とは限らない
4. `config()` は `new` / `new_*` に渡した要求値を返し、ネゴシエーション結果ではない（AVF は近傍の activeFormat、V4L2 はドライバによる調整、PipeWire は choice 交渉、MF はネイティブフォーマットへフォールバックしうる）
5. `start()` の「他バックエンドでは即座に復帰する」は AVF では不正確（`startRunning` はブロックする）
6. `VideoFormat` の `min_fps` / `max_fps` はバックエンド依存の概算である（PipeWire は固定 1.0 / 30.0、V4L2 は stepwise の最大サイズ 1 件 + 取得失敗時 1.0 / 30.0、MF は nominal で min = max）
7. `VideoFrame::data` の `len()` は、FFI では NV12 / I420 / YUY2 が `stride * height`（行パディング込み）、MJPEG が圧縮ペイロード長。MF では NV12 / I420 が `width * height`、YUY2 が `width * 2 * height`。rustdoc に長さの意味が無い
8. `VideoCapture` を drop すると running なら `stop()` と同等のブロッキング停止が走る。rustdoc に無い
9. `new_avf` と macOS の `new()` は権限未決定時に OS のダイアログが出て、許可・拒否まで戻らない。rustdoc に無い
10. `Error` の variant の発生源はプラットフォーム依存（`ComInitFailed` / `CaptureFaulted` は MF、`InvalidUtf8` は FFI 中心）。variant 自体は全プラットフォームで定義される。同じ原因が別 variant になることもある。rustdoc に記載が無い
11. `PixelFormat::Unknown` は公開 variant だが、現状どのバックエンドも生成しない（各バックエンドの列挙・変換で未知フォーマットを除外している。FFI は C 側、MF は `guid_to_pixel_format`）。rustdoc に「現状は生成されない」ことを書くか、将来のバックエンド追加用であることを明記する

## 優先度根拠

- Medium。コードは動くが契約が実態と一致せず、クロスプラットフォーム利用時に誤用を招く
- `/review-code` の重要指摘として確認済み

## 現状

- 1〜11 のバックエンド差は、`src/capture.rs` / `src/device.rs` / `src/error.rs` / `src/types.rs` の rustdoc に契約として整理されていない。一部は既に記載されているが、不足や実装との食い違いがある
- 既存の rustdoc にはバックエンド固有の記述がある（`src/capture.rs` の `start` / `stop`、`src/types.rs` の `VideoCaptureConfig` / `PixelFormat::Mjpeg` / `PixelBuffer`、`src/device.rs` の `name` / `unique_id` など）。
  ただし 1〜11 の契約としては不十分
- `skills/shiguredo-video-device/SKILL.md` には 1〜11 のバックエンド差の大部分が記載されており、README にも一部が記載されている。ただし rustdoc には反映されていない
- `src/capture.rs` の `start` の rustdoc は「他バックエンドでは即座に復帰する」としているが、AVF の `startRunning` はブロックするため実装と一致していない

## 設計方針

- 変更対象は `src/capture.rs` / `src/device.rs` / `src/error.rs` / `src/types.rs` の公開 API の rustdoc。必要なら `src/lib.rs` のクレート doc も更新する
- 各公開アイテムの rustdoc に、上記 1〜11 の該当するバックエンド差を表または箇条書きで追記する
- 既存の rustdoc に実装と食い違う記述があれば修正する（`start()` の「他バックエンドでは即座に復帰する」など）
- `VideoFormat` / `VideoFrame` の値と `timestamp_us` はバックエンド依存であること、`timestamp_us` は絶対時刻として使えないことを明記する
- `VideoCapture` の Send / Sync と drop の挙動を明記する
- `config()` は要求値であることを明記し、ネゴシエーション結果はフレームの各フィールドを参照するよう案内する
- `VideoDevice::unique_id()` の値を `VideoCaptureConfig::device_id` に渡せること、`device_id: None` の既定がバックエンドで異なることを明記する
- `VideoFormat` の `min_fps` / `max_fps` が概算であること、`VideoFrame::data` / `uv_data` の長さの意味、`Error` の各 variant の発生源、`PixelFormat::Unknown` が現状生成されないことを明記する
- `VideoCapture::new()` / `new_avf()` が macOS で権限ダイアログを出すことを明記する

## 完了条件

- 上記 1〜11 がすべて公開 API の rustdoc に記載されている
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` が通る
- doc の記述と実装が一致している（レビューで確認）

## 解決方法

{完了時に記入}
