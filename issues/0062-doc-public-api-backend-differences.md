# 公開 API のバックエンド差を doc に明記する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/update-public-api-doc-backend-differences
- Polished: {YYYY-MM-DD}

## 目的

公開 API の契約がバックエンド（macOS AVF / Linux V4L2 / Linux PipeWire / Windows MF）で異なるのに、doc がその差を説明していない。クロスプラットフォームで使う利用者が誤用する。

1. `VideoCapture` の auto trait: macOS / Linux は `Send` だが、Windows は `MfCaptureImpl` が `CoInitGuard`（`!Send`）を保持するため `!Send`。`Sync` は全バックエンドで無い。doc に記載が無い
2. `timestamp_us` の起点: AVF はセッションの PTS、V4L2 はドライバのタイムスタンプ、MF はストリーム相対の 100ns、PipeWire はグラフ時刻 / `CLOCK_MONOTONIC`。doc は「マイクロ秒」としか書いていない
3. `device_id` の解決規則: AVF は `uniqueID`、V4L2 は `/dev/videoN`、MF はシンボリックリンク、PipeWire は node name。`unique_id()` をそのまま渡せることが書かれていない。`None` の既定も V4L2 は `/dev/video0` 固定で、列挙結果の先頭ではない
4. `config()` は `new` に渡した要求値を返し、ネゴシエーション結果ではない（AVF は近傍の activeFormat、PipeWire は choice 交渉、MF はネイティブフォーマットへフォールバックしうる）
5. `start()` の「他バックエンドでは即座に復帰する」は AVF では不正確（`startRunning` はブロックする）
6. `VideoFormat` の `min_fps` / `max_fps` はバックエンド依存の概算である（PipeWire は固定 1.0 / 30.0、V4L2 は stepwise の最大サイズ 1 件 + 取得失敗時 1.0 / 30.0、MF は nominal で min = max）
7. `VideoFrame::data` の `len()` は FFI では `stride * height`（行パディング込み）だが、MF では `width * height`。doc に長さの意味が無い
8. `VideoCapture` を drop すると running なら `stop()` と同等のブロッキング停止が走る。doc に無い
9. `new_avf` は権限未決定時に OS のダイアログが出て、許可・拒否まで戻らない。doc に無い
10. `Error` の variant はプラットフォーム依存（`ComInitFailed` / `CaptureFaulted` は MF、`InvalidUtf8` は FFI 中心）。同じ原因が別 variant になることもある
11. `PixelFormat::Unknown` は公開 variant だが、現状どのバックエンドも生成しない（C 側で未知フォーマットを除外している）。doc に「現状は生成されない」ことを書くか、将来のバックエンド追加用であることを明記する

## 優先度根拠

- Medium。コードは動くが契約が実態と一致せず、クロスプラットフォーム利用時に誤用を招く
- `/review-code` の重要指摘として確認済み

## 現状

`src/capture.rs` / `src/device.rs` / `src/error.rs` / `src/types.rs` の doc は共通の説明のみで、バックエンド差に触れていない。

## 設計方針

- 各公開アイテムの doc にバックエンド差を表または箇条書きで追記する
- `VideoFormat` / `VideoFrame` / `timestamp_us` は「絶対時刻として使えない」「バックエンド依存」であることを明記する
- `VideoCapture` の Send / Sync と drop の挙動を明記する
- `config()` は要求値であることを明記し、ネゴシエーション結果はフレームの各フィールドを参照するよう案内する

## 完了条件

- 上記 1〜11 がすべて公開 doc に記載されている
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` が通る
- doc の記述と実装が一致している（レビューで確認）

## 解決方法

{完了時に記入}
