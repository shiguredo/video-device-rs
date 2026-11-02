# MF のキャプチャスレッドが panic 後に再 start で成功を返し、ReadSample のエラーを無限リトライする

- Priority: High
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-mf-capture-thread-hardening
- Polished: {YYYY-MM-DD}

## 目的

`src/capture_mf.rs` のキャプチャスレッドの状態管理とフレーム読み出しに、無音故障となる問題がある。

1. ユーザーコールバックが panic してキャプチャスレッドが終了しても `running` は真のまま残る。`stop()` を挟まずに `start()` を呼ぶと、`running` の確認だけで `Ok(())` を返し、コールバックも回収されない。README は「panic 後の再 start は `Error::CaptureFaulted`」と無条件に書いているが、実際に `CaptureFaulted` になるのは `stop()` を呼んだ後だけ
2. `ReadSample` がエラーを返すと 1ms スリープで無限にリトライし、エラーも終了も通知しない。`flags` を受け取っているが `MF_SOURCE_READERF_ENDOFSTREAM` / `MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED` を判定していない
3. `ReadSample` はキャンセルできないため、カメラがフレーム供給を止めると `stop()` の `join` が長時間ブロックする可能性がある
4. フレームの stride を常に `width`（YUY2 は `width * 2`）と仮定しており、`MF_MT_DEFAULT_STRIDE` を参照していない。パディング付き stride を返すデバイスで画像が崩れる
5. `get_device_symbolic_link` / `get_device_string` はデバイス由来の `GetStringLength` の値で `Vec<u16>` を確保する。極端に大きい値を返す属性に対する上限が無い
6. `MfCaptureImpl::stop` はキャプチャスレッドが panic したとき `eprintln!` でライブラリから直接 stderr へ書く。他バックエンドは同種の診断を出さず、利用側で抑制・収集できない

## 優先度根拠

- High。(1)(2) は「start 成功なのにフレームが来ない / エラーが続く」無音故障で、利用者が原因を特定できない
- (4) は Windows 実機でのみ発生するが、画像破綻に直結する
- 既存の MF 状態管理のハードニング issue は COM 初期化失敗経路と Drop 順序のみを対象としており、本 issue はユーザーコールバック panic と読み出しエラーを対象とする
- `/review-code` の重要指摘として確認済み

## 現状

`MfCaptureImpl::start` は `running` が真なら即 `Ok(())` を返す。panic したスレッドの終了を検知する仕組みが無い。`capture_thread_func` の `ReadSample` エラー分岐は次のとおり。

```rust
if result.is_err() {
    thread::sleep(Duration::from_millis(1));
    continue;
}
```

## 設計方針

- キャプチャスレッド終了時に `running` を必ず false にする RAII ガードをスレッド側に置く、または `start()` で `capture_thread` の終了を確認する
- `stop()` を挟まない再 `start()` も `Error::CaptureFaulted` を返すようにし、README の記述と実装を一致させる
- `flags` を判定し、EOS / エラー時は状態を遷移させて停止する
- `MF_MT_DEFAULT_STRIDE` を取得して stride を検証し、`width` と一致しない場合はフレームを破棄するか stride を反映する
- 文字列属性の長さに上限を設ける

## 完了条件

- コールバック panic 後に `stop()` の有無にかかわらず、再 `start()` が `Error::CaptureFaulted` を返す
- デバイス切断で `ReadSample` が失敗し続ける場合、無限リトライせず状態が変わり、利用者が検知できる
- パディング付き stride のデバイスで画像が崩れない（Windows 実機で確認）
- README / doc の記述と実装が一致する
- `cargo clippy --target x86_64-pc-windows-msvc --no-default-features --features default-mf --lib -- -D warnings` が通る

## 解決方法

{完了時に記入}
