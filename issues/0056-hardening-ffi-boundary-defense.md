# FFI 境界の防御と安全性注記を強化する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-ffi-boundary-defense
- Polished: {YYYY-MM-DD}

## 目的

FFI 境界に、契約違反時に未定義動作へ発展しうる防御漏れと、実態と合わない安全性注記がある。

1. `src/device_ffi.rs` の `FfiDeviceListImpl::enumerate` は `count` が負の場合に `count as usize` で巨大な値になり、`devices_ptr.add(i)` が境界外参照になる。`count > 0` かつ `devices_ptr` が NULL の場合も同様。`format_count` 側は `count.max(0)` で防御しており非対称
2. 同関数は `ret < 0` のとき、C 側が `devices_ptr` を確保していた場合の解放責任が曖昧（コメントと実装が矛盾）
3. `src/capture_ffi.rs` の `frame_callback` は `stride >= width` / `stride_uv >= ceil(width/2)` を検証しない。壊れた producer が小さい stride を返すと、幅より短いスライスを持つ `VideoFrame` が作られ、利用側の読み出しで panic する
4. `src/capture_mf.rs` の `SendPtr<T>` は任意の `T` に `unsafe impl Send` を与える。現在は `IMFSourceReader` 専用だが、将来 `!Send` の型を包むと未定義動作になる
5. `src/capture_ffi.rs` の `unsafe impl Send for FfiCaptureImpl` の SAFETY コメントは「専用スレッド上で動作するため」としているが、Send の根拠としては不正確。`FfiCaptureImpl::new` の SAFETY コメントも「config の width/height/fps は C 側で検証済み」と書いており、V4L2 / AVF は検証していない
6. `src/device.rs` の `VideoDeviceListInner::Ffi` は `_inner`（C の配列の所有権）と `devices`（その要素を指す `VideoDevice` の Vec）を保持しており、フィールドの drop 順が「C 配列の解放 → `VideoDevice` の drop」に依存している。現状 `VideoDevice` に `Drop` は無いため無害だが、将来 `Drop` を追加すると use-after-free になる
7. C の 3 バックエンドのデバイス列挙は `strdup` の戻り値を確認していない。メモリ不足時は名前が NULL のままデバイスが登録され、後から `Error::NullPointer` になる

## 優先度根拠

- Medium。現行の C 実装は契約を守っているため直ちに発生はしないが、FFI 境界の防御と安全性主張の正確性は Rust ライブラリとして維持すべき
- `/review-code` の指摘として確認済み

## 現状

- `src/device_ffi.rs` の `enumerate` は `(0..count as usize)` をそのまま回す
- `src/capture_ffi.rs` の `frame_callback` は `stride > 0` などの下限チェックのみ
- `src/capture_mf.rs` に `unsafe impl<T> Send for SendPtr<T> {}` がある

## 設計方針

- `count` を変換前に i32 として検証し、負値・NULL 配列を明示的にエラーにする
- `ret < 0` 時に非 NULL の `devices_ptr` を `free_devices` で解放してから返す
- `stride < width` / `stride_uv < ceil(width/2)` のフレームを破棄する
- `SendPtr<T>` を具象型専用（例: `SendSourceReader`）にする
- SAFETY コメントを実際の根拠（C のセッション API が任意スレッドから呼べる、コールバックは C 側の 1 スレッドに閉じる等）へ書き直す

## 完了条件

- 負の `count` や NULL 配列に対して未定義動作を起こさない
- 不正 stride のフレームが `VideoFrame` にならない
- `SendPtr` の安全性主張が型で限定されている
- SAFETY コメントがコードの実態と一致している
- `cargo clippy --workspace --all-targets -- -D warnings` が通る

## 解決方法

{完了時に記入}
