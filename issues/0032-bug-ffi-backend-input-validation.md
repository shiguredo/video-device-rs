# FFI バックエンド（AVF / V4L2 / PipeWire）に width / height / fps のバリデーションがない

- Priority: High
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-ffi-backend-input-validation
- Polished: 2026-11-04

## 目的

Windows バックエンドには `src/capture_mf.rs` の `validate_capture_config_for_windows` で width / height / fps の正値バリデーションがあるが、FFI バックエンド（AVF / V4L2 / PipeWire）には同等のバリデーションがない。AVF の `src/video_avf.m` の `video_avf_session_create` で `fps=0` を渡すと `CMTime frameDuration = CMTimeMake(1, fps);` が timescale=0 の不正 CMTime を生成し、同関数内の `CMTimeCompare` でクラッシュまたは未定義動作を引き起こす。V4L2 の `src/video_v4l2.c` の `video_v4l2_session_create` でも `fps <= 0` で `timeperframe.denominator` が 0 または負の不正パラメータを ioctl に渡す。FFI バックエンド共通のバリデーションを追加する。

## 優先度根拠

- High。AVF で `fps=0` はクラッシュ / UB の経路（致命的）
- V4L2 / PipeWire でも不正な ioctl パラメータや PipeWire への不正な fraction 渡しが発生する
- Windows 側にはバリデーションが存在し、FFI 側だけ欠落している不整合
- `/review-code` の致命的指摘（AVF fps）および重要指摘（V4L2 fps）として確認

## 現状

### AVF（致命的）

`src/video_avf.m` の `video_avf_session_create` に引数バリデーションがない。`fps=0` のとき:

```c
CMTime frameDuration = CMTimeMake(1, fps);  // timescale=0 → 不正 CMTime
```

同関数内の `CMTimeCompare` 呼び出しでクラッシュまたは未定義動作。

`width=0` / `height=0` の場合、同関数内の `output.videoSettings` の `kCVPixelBufferWidthKey` / `kCVPixelBufferHeightKey` に 0 が渡され、AVFoundation が 0x0 のピクセルバッファを要求する。挙動が不定（デフォルトにフォールバックするか、0 サイズのフレームが配信されるか）。

### V4L2（重要）

`src/video_v4l2.c` の `video_v4l2_session_create` で `VIDIOC_S_PARM` を設定する箇所:

```c
parm.parm.capture.timeperframe.numerator = 1;
parm.parm.capture.timeperframe.denominator = fps;  // fps=0 → denominator=0
xioctl(fd, VIDIOC_S_PARM, &parm);  // 戻り値無視
```

`fps <= 0` のとき `timeperframe.denominator` が 0 または負の不正な fraction をドライバに渡す。クラッシュには至らないが不正な ioctl。`width=0` / `height=0` の場合、`VIDIOC_S_FMT` に 0 を渡すとドライバがデフォルト解像度にフォールバックするか拒否するかはドライバ依存。

### PipeWire（軽微）

`src/video_pipewire.c` の `video_pipewire_session_create` で `width <= 0` / `height <= 0` / `fps <= 0` をデフォルト値にフォールバックしている。PipeWire 側は防御済みだが、Rust 側で拒否する方が一貫性がある。

**後方互換への影響**: 現在 PipeWire では `width=0` を渡すと C 側が 640 にフォールバックして正常に動作する。修正後は Rust 側で `Err` を返すようになる。これは breaking change だが、不正値の黙認はバグであり、`[FIX]` として扱う。

### Rust 側

`src/capture_ffi.rs` の `FfiCaptureImpl::new` に width / height / fps のバリデーションがない。`FfiCaptureImpl::new` は `new_avf` / `new_v4l2` / `new_pipewire` から呼ばれ、公開 API の `VideoCapture::new_avf` / `VideoCapture::new_v4l2` / `VideoCapture::new_pipewire` がこれらに委譲する。

## 設計方針

`src/capture_ffi.rs` の `FfiCaptureImpl::new` にバリデーションを追加する。

1. `FfiCaptureImpl::new` の冒頭（既存の `PixelFormat::Unknown` チェックより前）で `config.width <= 0 || config.height <= 0 || config.fps <= 0` をチェックし、`Err(Error::InvalidCaptureConfig(...))` を返す。上限チェック（`i32::MAX` 等）は本 issue のスコープ外
2. エラーメッセージは英語（AGENTS.md 規約）。例: `"width, height, and fps must be positive integers"`
3. `Error::InvalidCaptureConfig` は `src/error.rs` に既に存在する。Windows 側と同一のバリアントを再利用する
4. PipeWire の C 側フォールバックは Rust 側で拒否した上で C 側の防御も残す（二重防御）
5. `VideoCaptureConfig` の doc コメントを更新する。現在の「**Windows** では `width` / `height` / `fps` はいずれも正の整数である必要がある（Media Foundation への渡し方のため）。」と「**Linux（PipeWire 等）** では不正値を C 側が既定解像度・フレームレートに置き換える場合があるため、Rust 側では拒否しない。」の 2 行を削除し、「全プラットフォームで width / height / fps は正の整数である必要がある。不正値は `Error::InvalidCaptureConfig` を返す」に置き換える
6. `FfiCaptureImpl::new` の `unsafe` ブロック直前の SAFETY コメント「config の width/height/fps はユーザー指定値であり、C 側で検証済み。」は事実と異なる（AVF/V4L2 の C 側は検証しない）。バリデーション追加後に「Rust 側で検証済み」に修正する。SAFETY コメントは `unsafe` ブロックの正当性の根拠であり、間違ったままにすると将来のレビュアーが「C 側で検証しているなら Rust 側のバリデーションは冗長では？」と誤解するため、本修正の一部として扱う
7. `README.md` の `VideoCaptureConfig` の注意点にある「Windows では `width` / `height` / `fps` は正の整数である必要があります。違反すると `Error::InvalidCaptureConfig` を返します」も、全プラットフォームの記述に更新する

バックエンド固有のピクセルフォーマット拒否（AVF / PipeWire の MJPEG 非対応など）は `new_avf` / `new_v4l2` / `new_pipewire` 側で `FfiCaptureImpl::new` より先に評価される。不正な width / height / fps とこれらの未対応ピクセルフォーマットが同時に指定された場合は `UnsupportedPixelFormat` が優先される。`FfiCaptureImpl::new` 内の `PixelFormat::Unknown` チェックはバリデーションより後なので、`Unknown` と不正値が同時に指定された場合は `InvalidCaptureConfig` が優先される。

### 採用しない案

- C 側だけでバリデーションする: Rust 側で早期に拒否する方がエラーメッセージが明確で、C 側の未定義動作を未然に防げる
- `VideoCaptureConfig::validate()` メソッドを追加する: 公開 API の変更を伴う。`FfiCaptureImpl::new` 内の非公開バリデーションで十分

## 完了条件

- `FfiCaptureImpl::new` で width / height / fps の正値バリデーションを追加する
- `fps=0` / `width=0` / `height=0` / 負値に対して `Err(Error::InvalidCaptureConfig(...))` を返す。ただし `new_avf` / `new_v4l2` / `new_pipewire` が先に `UnsupportedPixelFormat` を返す場合（AVF / PipeWire の MJPEG 非対応など）は、そのエラーが優先される
- `FfiCaptureImpl::new` の SAFETY コメントを「Rust 側で検証済み」に修正する
- `VideoCaptureConfig` の doc コメントを更新する
- `README.md` の `VideoCaptureConfig` の注意点を全プラットフォームの記述に更新する
- `cargo build --workspace` が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- バリデーションの単体テストを追加する（`Error::InvalidCaptureConfig` が返ること）。`tests/test_capture.rs` に配置し、`VideoCapture::new_avf()` / `VideoCapture::new_v4l2()` / `VideoCapture::new_pipewire()` を有効なバックエンドごとに `#[cfg(enable_avf)]` / `#[cfg(enable_v4l2)]` / `#[cfg(enable_pipewire)]` で個別にテストする（ファイル全体は既に `any(enable_avf, enable_v4l2, enable_pipewire, enable_mf)` でゲートされている）。バリデーションは FFI 呼び出しより前に `Err` を返すため、実際のカメラアクセスは発生せず、モック・スタブ不要でテスト可能
- `tests/test_capture.rs` のモジュール doc に、実機デバイス不要のバリデーションテストを含む旨を追記する
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
