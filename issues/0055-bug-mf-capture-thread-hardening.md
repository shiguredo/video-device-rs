# MF のキャプチャスレッドが panic 後に再 start で成功を返し、ReadSample のエラーを無限リトライする

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-mf-capture-thread-hardening
- Polished: 2026-11-05

## 目的

MF バックエンドのキャプチャスレッドの状態管理・フレーム読み出しと、デバイス文字列の取得に、無音故障やリソース消費の問題がある。

1. ユーザーコールバックが panic してキャプチャスレッドが終了しても `running` は真のまま残る。`stop()` を挟まずに `start()` を呼ぶと、`running` の確認だけで `Ok(())` を返し、コールバックも回収されない。`src/capture.rs` と `src/lib.rs` の rustdoc は `stop()` の有無を区別せず `Error::CaptureFaulted` と書いており、実装と一致していない。
2. `ReadSample` がエラーを返すと 1ms スリープで無限にリトライし、エラーも終了も通知しない。`flags` を受け取っているが `MF_SOURCE_READERF_ERROR` / `MF_SOURCE_READERF_ENDOFSTREAM` / `MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED` を判定していない。
3. `ReadSample` は同期モードでは次のサンプルが来るまでブロックするため、カメラがフレーム供給を止めると `stop()` の `join` が長時間ブロックする可能性がある。
4. `process_sample` は `IMFMediaBuffer::Lock` で取得したデータの stride を常に `width`（YUY2 は `width * 2`）と仮定しており、`MF_MT_DEFAULT_STRIDE` を参照していない。最小ストライドが幅と一致しないフォーマット・デバイスでは画像が崩れる。
5. `get_device_symbolic_link` / `get_device_string` はデバイス由来の `GetStringLength` の値で `Vec<u16>` を確保する。極端に大きい値を返す属性に対する上限が無く、`length + 1` のオーバーフローも起こりうる。
6. `MfCaptureImpl::stop` はキャプチャスレッドが panic したとき `eprintln!` でライブラリから直接 stderr へ書く。他バックエンドは同種の診断を出さず、利用側で抑制・収集できない。

## 優先度根拠

- High。(1)(2) は「start 成功なのにフレームが来ない / エラーが続く」無音故障で、利用者が原因を特定できない
- (4) は最小ストライドが幅と一致しないフォーマット・デバイスで画像破綻に直結する。通常は `IMFMediaBuffer::Lock` が最小ストライドを返すため問題にならないが、前提が崩れた場合に備える
- (3) はフレーム供給停止時に `stop()` が長時間ブロックしうる。本 issue ではキャンセル機構を追加せず、`stop()` のブロッキング性を doc に明記する
- 既存の MF 状態管理のハードニング issue (0037) は COM 初期化失敗経路と Drop 順序を対象としており、本 issue はユーザーコールバック panic、読み出しエラー、stride、文字列長、診断を対象とする
- `/review-code` の重要指摘として確認済み

## 現状

`MfCaptureImpl::start` は `running` が真なら即 `Ok(())` を返す。

```rust
if self.running.load(Ordering::Acquire) {
    return Ok(());
}
```

`MfCaptureImpl::stop` は `running` が偽なら即 return し、`join` が `Err` のとき `eprintln!` する。

```rust
if !self.running.load(Ordering::Acquire) {
    return;
}
...
Err(_) => {
    eprintln!(
        "capture thread panicked; callback is permanently lost and capture cannot be restarted"
    );
}
```

`capture_thread_func` の `ReadSample` エラー分岐は次のとおり。

```rust
if result.is_err() {
    thread::sleep(Duration::from_millis(1));
    continue;
}
```

`flags` は受け取っているが、`MF_SOURCE_READERF_ERROR` / `MF_SOURCE_READERF_ENDOFSTREAM` / `MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED` を判定していない。

`process_sample` は NV12/I420 を `stride = width`、YUY2 を `stride = width * 2` としてスライスする。`MF_MT_DEFAULT_STRIDE` は参照していない。

```rust
let y_data = &data[..y_size];
let uv_data = &data[y_size..required];
...
stride: width,
```

`get_device_symbolic_link` と `get_device_string` は `GetStringLength` の値をそのまま `Vec<u16>` の長さに使う。

```rust
let mut buffer: Vec<u16> = vec![0; (length + 1) as usize];
```

ドキュメントの記述:

- `src/capture.rs` の `VideoCapture::start` は「キャプチャスレッドが panic したあとに再 `start` すると `CaptureFaulted`」と、`stop()` の有無を区別せず書いている。
- `src/lib.rs` も同様に「その後の再 `VideoCapture::start` が `CaptureFaulted`」と書いている。
- `README.md` は「`stop()` の後の再 `start()` が `CaptureFaulted`」と書いており、`stop()` を挟まない `start()` が `Ok(())` になることを書いていない。
- `skills/shiguredo-video-device/SKILL.md` は「`stop()` を挟まない `start()` は実行中とみなして `Ok(())` を返す」と現状の実装を書いている。

## 設計方針

### 1. キャプチャスレッドの終了状態と再 start

- `MfCaptureImpl` に、正常停止と異常終了を区別する fault 状態を持たせる。fault 状態は `Arc<AtomicBool>` などでキャプチャスレッドと共有し、スレッドが異常終了したときに真にする。
- `start()` は `running` の判定より先に fault 状態と `capture_thread` の終了を確認する。`fault` 状態なら `Error::CaptureFaulted` を返す。`capture_thread` が `Some` で `JoinHandle::is_finished()` が真なら `join()` する。`join()` が `Err` なら fault 状態にし、`Error::CaptureFaulted` を返す。`Ok(callback)` なら callback を回収して `capture_thread` を `None` にする。`capture_thread` が `Some` で `is_finished()` が偽の場合は、`running` が真なら `Ok(())` を返し、`running` が偽なら `join()` する（`join()` の結果は上と同じく処理する）。callback が `None` なら `Error::CaptureFaulted` を返す。スレッド起動前に `get_configured_format` を呼び直し、`SessionData` の pixel_format / width / height / stride を最新化する。取得に失敗したら `Error::SessionStartFailed`、`Unknown` なら `Error::UnsupportedPixelFormat` を返す。`running` を true にして新しいスレッドを起動する。
- `stop()` は `running` が真なら false にしてから、`running` の値にかかわらず `capture_thread` が `Some` なら `join()` する。`join()` が `Err` なら fault 状態にし、callback は回収しない。`Ok(callback)` なら callback を回収する。fault 状態は `stop()` では消さない（同じ `VideoCapture` を再利用しない契約を維持する）。
- `capture_thread_func` は、COM 初期化失敗、`ReadSample` の Err、`MF_SOURCE_READERF_ERROR`、`MF_SOURCE_READERF_ENDOFSTREAM`、現在のメディアタイプの再取得失敗で fault 状態にし、`running` を false にして callback を返す。正常な `stop()` によるループ終了では fault 状態にしない。
- panic 時は `start()` / `stop()` の `join()` が `Err` を返すことで fault 状態にする。スレッド側の RAII ガードは必須ではないが、`running` の残留を防ぐために置いてもよい。単独の RAII ガードでは、正常終了したスレッドの callback を回収できないため不十分である。
- これにより、panic 後に `stop()` を挟んでも挟まなくても、再 `start()` は `Error::CaptureFaulted` を返す。
- 正常な `stop()` 後の再 `start()` は許容し、fault 状態では `Error::CaptureFaulted` を返すことを README / rustdoc / skill に明記する。`stop()` の「running でなければ no-op」という既存の doc は、終了済みスレッドの `join` を含む記述に更新する。

### 2. ReadSample のエラーと flags

- `ReadSample` が Err を返したら、1ms スリープで再試行せず、fault 状態にして `running` を false にし、スレッドを終了する。
- `flags` を次のように判定する。サンプルが同時に返された場合は、そのサンプルを処理してから終了する。
  - `MF_SOURCE_READERF_ERROR`: それ以上 `IMFSourceReader` のメソッドを呼ばず、fault 状態にして終了する（MSDN の「エラー後は IMFSourceReader のメソッドを呼ばない」契約に従う）。
  - `MF_SOURCE_READERF_ENDOFSTREAM`: fault 状態にして終了する。
  - `MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED`: `get_configured_format` を呼び直して pixel_format / width / height / stride を更新する。`Unknown` や不正な値なら fault 状態にして終了する。
  - `MF_SOURCE_READERF_STREAMTICK` などサンプルを伴わないフラグは、そのままループを継続する。
- サンプルがある場合は従来どおり `process_sample` に渡す。

### 3. ReadSample のブロッキング

- `ReadSample` は同期モードでは次のサンプルが来るまでブロックする（MSDN）。`stop()` は `running` を false にして `join()` するが、`ReadSample` がブロック中だと次のサンプル到着まで待つ。
- 本 issue ではキャンセル機構を追加しない。`VideoCapture::stop` の rustdoc と README に、Windows では `stop()` が次のサンプル到着までブロックしうることを明記する。キャンセル可能な停止が必要なら別 issue とする。

### 4. stride

- `get_configured_format` で `MF_MT_DEFAULT_STRIDE` を取得し、pixel_format / width / height と一緒に返す。`SessionData` に stride を追加し、`start()` と `capture_thread_func` に渡す。
- `process_sample` では、フォーマットごとの最小ストライド（NV12/I420: `width`、YUY2: `width * 2`）を計算する。`MF_MT_DEFAULT_STRIDE` が無い場合は最小ストライドを使う。
- 取得した stride が負、または最小ストライド未満の場合は、そのフレームを破棄する（`IMFMediaBuffer::Lock` が返す最小ストライドの契約と矛盾するため）。
- stride が最小ストライド以上の場合は、stride を反映して `VideoFrame` を構築する。NV12 は UV プレーンのオフセットを `stride * height`、UV ストライドを `stride` とする。I420 は UV プレーンのオフセットを `stride * height`、UV ストライドを `stride / 2`（偶数であることを検証する）とする。YUY2 は `data` の長さを `stride * height` とする。`stride * height` などの計算は `checked_mul` を使い、失敗時はフレームを破棄する。
- `current_length` が必要なプレーンサイズ以上であることを確認してからスライスする。

### 5. 文字列属性の長さ

- `get_device_symbolic_link` と `get_device_string` で `GetStringLength` の戻り値を検証する。Windows のデバイス名・シンボリックリンクは通常 4096 文字を超えないため、上限は 4096 文字（UTF-16 コード単位）とする。`length == 0` または `length > 4096` なら `None` を返す。
- `length + 1` は `checked_add` で計算し、オーバーフロー時は `None` を返す。
- 上限超過時は呼び出し側の既存フォールバック（`get_device_symbolic_link` はデバイス不一致、`get_device_string` は `Unknown Device` / `device_{n}`）に委ねる。

### 6. 診断

- `MfCaptureImpl::stop` の `eprintln!` を削除する。panic は次回 `start()` の `Error::CaptureFaulted` で通知する。
- `VideoCapture::stop` の rustdoc から stderr の記述を削除し、panic 時は callback を回収せず、次回 `start()` が `CaptureFaulted` になることを書く。
- `Error::CaptureFaulted` の doc と Display を、panic に限らずキャプチャスレッドが異常終了した場合に使う旨へ更新する。Display は例として "capture is faulted and cannot be restarted" のように panic 限定でない文言にする。
- 0037 が `capture_thread_func` に追加する `eprintln!` も、本 issue の「ライブラリから直接 stderr へ書かない」方針と重複する。0037 と 0055 の実装順に応じて、後から実装する側で削除する。

## 変更対象

- `src/capture_mf.rs`: `SessionData`、`MfCaptureImpl::start` / `stop` / `Drop`、`capture_thread_func`、`process_sample`、`get_configured_format`、`get_device_symbolic_link`
- `src/device_mf.rs`: `get_device_string`
- `src/error.rs`: `Error::CaptureFaulted` の doc / `Display`
- `src/capture.rs`: `VideoCapture::start` / `stop` の rustdoc
- `src/lib.rs`: クレートドキュメント
- `src/frame_math.rs`: MF 用の stride ベースのプレーンサイズ計算を追加する
- `README.md`: キャプチャのライフサイクルとフレームコールバックの制約
- `skills/shiguredo-video-device/SKILL.md`: `VideoCapture` のライフサイクル、フレームコールバックの制約、エラー表、Windows の注意、既知の制限事項
- `CHANGES.md`: `## develop` に [FIX] エントリを追加する

## 完了条件

- コールバック panic によるキャプチャスレッドの終了後に `stop()` の有無にかかわらず、再 `start()` が `Error::CaptureFaulted` を返す。
- `ReadSample` が Err を返した場合、または `MF_SOURCE_READERF_ERROR` / `MF_SOURCE_READERF_ENDOFSTREAM` を受けた場合、無限リトライせずスレッドを終了し、キャプチャスレッドの終了後の `start()` が `Error::CaptureFaulted` を返す。
- `MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED` を受けた場合、現在のメディアタイプを再取得して pixel_format / width / height / stride を更新する。再取得に失敗した場合は fault として停止する。
- `start()` はスレッド起動前に現在のメディアタイプを再取得し、`SessionData` の pixel_format / width / height / stride を最新化する。取得失敗時は `Error::SessionStartFailed`、`Unknown` は `Error::UnsupportedPixelFormat` を返す。
- `VideoCapture::stop` の rustdoc と README に、Windows では `ReadSample` の同期ブロッキングにより `stop()` が次のサンプル到着まで待ちうることを明記する。
- `MF_MT_DEFAULT_STRIDE` を取得し、フォーマットごとの期待値（NV12/I420: `width`、YUY2: `width * 2`）と比較する。属性が無い場合は期待値を使い、負または期待値未満の場合はフレームを破棄する。期待値以上の場合は stride を反映して `VideoFrame` を構築する。パディング付き stride のデバイスで画像が崩れない（Windows 実機で確認）。
- `get_device_symbolic_link` / `get_device_string` の `GetStringLength` に 4096 文字の上限を設け、超過時は `None` を返す。`length + 1` のオーバーフローも避ける。
- `MfCaptureImpl::stop` の `eprintln!` を削除し、`VideoCapture::stop` の rustdoc から stderr の記述を削除する。`Error::CaptureFaulted` の Display を panic 限定でない文言にする。
- README / `src/capture.rs` / `src/lib.rs` / `skills/shiguredo-video-device/SKILL.md` の記述を実装に合わせる。`stop()` の「running でなければ no-op」の記述は、終了済みスレッドの `join` を含む実装に合わせる。`skills/shiguredo-video-device/SKILL.md` の「`stop()` を挟まない `start()` は `Ok(())`」と「`stop()` 時に stderr へログを出す」も更新する。
- `cargo clippy --target x86_64-pc-windows-msvc --no-default-features --features default-mf --lib -- -D warnings` が通る。
- `cargo fmt --all -- --check` が通る。
- Windows 実機で `cargo test --workspace` が通る（既存テストに回帰がないこと）。
- `CHANGES.md` の `## develop` に [FIX] エントリを追加する（担当者行を含める）。

## 解決方法

{完了時に記入}
