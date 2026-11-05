# FFI 境界の防御と安全性注記を強化する

- Priority: Medium
- Created: 2026-11-02
- Completed: {YYYY-MM-DD}
- Model: deepseek-v4.1-flash:1017
- Branch: feature/fix-ffi-boundary-defense
- Polished: 2026-11-05

## 目的

FFI 境界に、契約違反時に未定義動作へ発展しうる防御漏れと、実態と合わない安全性注記がある。

1. `src/device_ffi.rs` の `FfiDeviceListImpl::enumerate` は `count` が負の場合に `count as usize` で巨大な値になり、`devices_ptr.add(i)` が境界外参照になる。`count > 0` かつ `devices_ptr` が NULL の場合も同様。`format_count` 側は `count.max(0)` で防御しており非対称
2. 同関数は `ret < 0` のとき、C 側が `devices_ptr` を確保していた場合の解放責任が曖昧（コメントと実装が矛盾）
3. `src/capture_ffi.rs` の `frame_callback` はフォーマットごとの最小ストライド（NV12 は Y / UV とも `width`、I420 は Y が `width`・UV が `ceil(width/2)`、YUY2 は `width * 2`）を検証しない。壊れた producer がこれより小さい stride を返すと、フォーマットが要求する行長より短いスライスを持つ `VideoFrame` が作られ、利用側が stride 前提で各行を読むと範囲外アクセスになり panic しうる
4. `src/capture_mf.rs` の `SendPtr<T>` は任意の `T` に `unsafe impl Send` を与える。現在は `IMFSourceReader` 専用だが、将来 `!Send` の型を包むと未定義動作になる
5. `src/capture_ffi.rs` の `unsafe impl Send for FfiCaptureImpl` の SAFETY コメントは「専用スレッド上で動作するため」としているが、Send の根拠としては不正確。`FfiCaptureImpl::new` の SAFETY コメントも「config の width/height/fps は C 側で検証済み」と書いており、V4L2 / AVF は検証していない
6. `src/device.rs` の `VideoDeviceListInner::Ffi` は `_inner`（C の配列の所有権）と `devices`（その要素を指す `VideoDevice` の Vec）を保持しており、フィールドの drop 順が「C 配列の解放 → `VideoDevice` の drop」に依存している。現状 `VideoDevice` に `Drop` は無いため無害だが、将来 `Drop` を追加すると use-after-free になる
7. C / Objective-C の 3 バックエンドのデバイス列挙は `strdup` の戻り値を確認していない。メモリ不足時は名前や一意識別子が NULL のままデバイスが登録され、後から `Error::NullPointer` になる

## 優先度根拠

- Medium。現行の C 実装は契約を守っているため直ちに発生はしないが、FFI 境界の防御と安全性主張の正確性は Rust ライブラリとして維持すべき
- `/review-code` の指摘として確認済み

## 現状

- `src/device_ffi.rs` の `enumerate` は `(0..count as usize)` をそのまま回す
- `src/capture_ffi.rs` の `frame_callback` は `stride > 0` などの下限チェックのみ
- `src/capture_mf.rs` に `unsafe impl<T> Send for SendPtr<T> {}` がある
- `src/capture_ffi.rs` の `unsafe impl Send for FfiCaptureImpl` の SAFETY コメントは「専用スレッド上で動作するため」、`FfiCaptureImpl::new` の SAFETY コメントは「C 側で検証済み」としている
- `src/device.rs` の `VideoDeviceListInner::Ffi` は `_inner` を先に宣言している
- `src/video_avf.m` / `src/video_v4l2.c` / `src/video_pipewire.c` のデバイス列挙は `strdup` の戻り値を確認していない

## 設計方針

- `count` を `as usize` に変換する前に検証する。`count < 0` は列挙失敗として扱う。`count > 0` かつ `devices_ptr` が NULL の場合は `Error::NullPointer("devices")` を返す。`count == 0` は `devices_ptr` の NULL / 非 NULL にかかわらず正常な空リストとして扱い、非 NULL の場合は `Drop` で配列を解放する。負の `count` は、0052 のエラーコードマッピングが実装済みなら `Error::BackendUnavailable`、未実装なら既存の `Error::DeviceAccessDenied` を返す。新しい `Error` variant は追加しない（`Error` は `#[non_exhaustive]` ではないため、variant の追加は後方互換のない変更になる）
- `ret < 0` のとき、`devices_ptr` が非 NULL なら `free_devices(devices_ptr, count)` で解放してからエラーを返す。`count` は C 側が確保した要素数として有効であることを前提とし、C 側は失敗時に `devices_ptr` を設定する場合、配列を解放せず Rust に委ねる（現行 C は失敗時に `devices_ptr` を設定しないため変更不要）。`count < 0` の場合は要素数を信用できないため、`free_devices(devices_ptr, 0)` で配列だけ解放する
- `frame_callback` で、既存の `frame_math` の下限チェックを通った後、`from_raw_parts` の前にフォーマットごとの最小ストライドを検証し、満たさないフレームは `VideoFrame` を作らずに破棄する。`PixelBuffer` は既存どおり drop する。NV12: `stride >= width` かつ `stride_uv >= width`。I420: `stride >= width` かつ `stride_uv >= width.div_ceil(2)`。YUY2: `stride >= width * 2`（`stride_uv` は 0 のため検証しない）。MJPEG: `stride` はペイロード長であり行ストライドではないため検証しない。`width` は正の i32 であることを確認済みなので、比較は `usize` に変換してから行い、`width * 2` の i32 オーバーフローを避ける
- `SendPtr<T>` を廃止し、`IMFSourceReader` 専用の具象ラッパー（例: `SendSourceReader`）にする。`unsafe impl Send` はその具象型にだけ与え、`IMFSourceReader` を MTA で生成し、MTA で初期化したキャプチャスレッドで使用することを SAFETY コメントに書く
- `unsafe impl Send for FfiCaptureImpl` の SAFETY コメントを、実際の根拠（C のセッション API は作成スレッド以外から `start` / `stop` / `destroy` を呼べる、`CaptureContext` のコールバックは `Box<dyn Fn + Send>` でスレッド間移動できる、C 側のコールバックは 1 スレッドに閉じる等）へ書き直す。`FfiCaptureImpl::new` の SAFETY コメントから「config の width/height/fps は C 側で検証済み」という実態と合わない主張を削除する（Rust 側のバリデーション追加は 0032 の担当）
- `VideoDeviceListInner::Ffi` のフィールド順を `devices` → `_inner` に変更し、`devices` が `_inner` より先に drop されるようにする。これにより、将来 `VideoDevice` に `Drop` を追加しても C 配列の解放後に `VideoDevice` が drop されることがない
- C / Objective-C の 3 バックエンドのデバイス列挙で `strdup` の戻り値を確認する。`name` または `unique_id` の `strdup` が NULL を返した場合は、そのデバイスを配列に登録せず、確保済みの `name` / `unique_id` / `formats` / デバイス本体を解放して列挙を継続する。登録するデバイスだけを配列に詰め、`*count` は実際に登録した数にする（AVF の `deviceArray[i]` と `*count = deviceCount` は詰め直しが必要）。列挙全体を失敗させず、該当デバイスだけをスキップする（OOM 時の 1 デバイス欠落は許容し、NULL 名の登録を避ける）

## 完了条件

- 負の `count`、`count > 0` の NULL 配列、`count == 0` の空リスト（`devices_ptr` の NULL / 非 NULL を問わない）の扱いが設計方針どおりで、未定義動作を起こさない
- `ret < 0` で C 側が確保した `devices_ptr` が解放される（リークしない）
- 不正 stride（NV12 / I420 / YUY2 の各最小ストライド未満）のフレームが `VideoFrame` にならない
- `SendPtr<T>` が廃止され、`IMFSourceReader` 専用の具象ラッパーに `unsafe impl Send` が限定されている
- `unsafe impl Send for FfiCaptureImpl` と `FfiCaptureImpl::new` の SAFETY コメントがコードの実態と一致している
- `VideoDeviceListInner::Ffi` の drop 順が `devices` → `_inner` になり、将来 `VideoDevice` に `Drop` を追加しても C 配列の解放後に触れない
- `strdup` に失敗したデバイスが NULL 名または NULL の一意識別子のまま登録されず、配列に穴が残らず、`*count` が実際に登録した数と一致する
- `cargo clippy --workspace --all-targets -- -D warnings` が通る

## 解決方法

{完了時に記入}
