# FFI バックエンド（AVF / V4L2 / PipeWire）に width / height / fps のバリデーションがない

- Priority: High
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-ffi-backend-input-validation
- Polished: 2026-07-21

## 目的

Windows バックエンドには `validate_capture_config_for_windows`（`src/capture_mf.rs:49-58`）で width / height / fps の正値バリデーションがあるが、FFI バックエンド（AVF / V4L2 / PipeWire）には同等のバリデーションがない。AVF で `fps=0` を渡すと `CMTimeMake(1, 0)`（`src/video_avf.m:433`）が timescale=0 の不正 CMTime を生成し、`CMTimeCompare` でクラッシュまたは未定義動作を引き起こす。V4L2 でも `fps <= 0` で `timeperframe.denominator = 0` の不正パラメータを ioctl に渡す。FFI バックエンド共通のバリデーションを追加する。

## 優先度根拠

- High。AVF で `fps=0` はクラッシュ / UB の経路（致命的）
- V4L2 / PipeWire でも不正な ioctl パラメータや PipeWire への不正な fraction 渡しが発生する
- Windows 側にはバリデーションが存在し、FFI 側だけ欠落している不整合
- `/review-code` の致命的指摘（AVF fps）および重要指摘（V4L2 fps）として確認

## 現状

### AVF（致命的）

`src/video_avf.m:320-322` の `video_avf_session_create` に引数バリデーションがない。`fps=0` のとき:

```c
CMTime frameDuration = CMTimeMake(1, fps);  // timescale=0 → 不正 CMTime
```

以降の `CMTimeCompare`（:438-439）でクラッシュまたは未定義動作。

`width=0` / `height=0` の場合、`kCVPixelBufferWidthKey` / `kCVPixelBufferHeightKey`（:400-405）に 0 が渡され、AVFoundation が 0x0 のピクセルバッファを要求する。挙動が不定（デフォルトにフォールバックするか、0 サイズのフレームが配信されるか）。

### V4L2（重要）

`src/video_v4l2.c:494-496`:

```c
parm.parm.capture.timeperframe.numerator = 1;
parm.parm.capture.timeperframe.denominator = fps;  // fps=0 → denominator=0
xioctl(fd, VIDIOC_S_PARM, &parm);  // 戻り値無視
```

不正な fraction をドライバに渡す。クラッシュには至らないが不正な ioctl。`width=0` / `height=0` の場合、`VIDIOC_S_FMT` に 0 を渡すとドライバがデフォルト解像度にフォールバックするか拒否するかはドライバ依存。

### PipeWire（軽微）

`src/video_pipewire.c:720-728` で `width <= 0` / `height <= 0` / `fps <= 0` をデフォルト値にフォールバックしている。PipeWire 側は防御済みだが、Rust 側で拒否する方が一貫性がある。

**後方互換への影響**: 現在 PipeWire では `width=0` を渡すと C 側が 640 にフォールバックして正常に動作する。修正後は Rust 側で `Err` を返すようになる。これは breaking change だが、不正値の黙認はバグであり、`[FIX]` として扱う。

### Rust 側

`src/capture_ffi.rs:80-129` の `FfiCaptureImpl::new()` に width / height / fps のバリデーションがない。

## 設計方針

`src/capture_ffi.rs` の `FfiCaptureImpl::new()` にバリデーションを追加する。

1. `FfiCaptureImpl::new()` の冒頭（既存の `PixelFormat::Unknown` チェック（:86-92）より前）で `config.width <= 0 || config.height <= 0 || config.fps <= 0` をチェックし、`Err(Error::InvalidCaptureConfig(...))` を返す。上限チェック（`i32::MAX` 等）は本 issue のスコープ外
2. エラーメッセージは英語（AGENTS.md 規約）。例: `"width, height, and fps must be positive integers"`
3. `Error::InvalidCaptureConfig` は既に存在する（`src/error.rs:12`）。Windows 側と同一のバリアントを再利用する
4. PipeWire の C 側フォールバック（:720-728）は Rust 側で拒否した上で C 側の防御も残す（二重防御）
5. `VideoCaptureConfig` の doc コメント（`src/types.rs:179-184`）を更新する。現在の :181（「**Windows** では...正の整数である必要がある」）と :182（「**Linux（PipeWire 等）** では...Rust 側では拒否しない」）の 2 行を削除し、「全プラットフォームで width / height / fps は正の整数である必要がある。不正値は `Error::InvalidCaptureConfig` を返す」に置き換える
6. `capture_ffi.rs:105` の SAFETY コメント「config の width/height/fps はユーザー指定値であり、C 側で検証済み」は事実と異なる（AVF/V4L2 の C 側は検証しない）。バリデーション追加後に「Rust 側で検証済み」に修正する。SAFETY コメントは `unsafe` ブロックの正当性の根拠であり、間違ったままにすると将来のレビュアーが「C 側で検証しているなら Rust 側のバリデーションは冗長では？」と誤解するため、本修正の一部として扱う

### 採用しない案

- C 側だけでバリデーションする: Rust 側で早期に拒否する方がエラーメッセージが明確で、C 側の未定義動作を未然に防げる
- `VideoCaptureConfig::validate()` メソッドを追加する: 公開 API の変更を伴う。`FfiCaptureImpl::new()` 内の非公開バリデーションで十分

## 完了条件

- `FfiCaptureImpl::new()` で width / height / fps の正値バリデーションを追加する
- `fps=0` / `width=0` / `height=0` / 負値で `Err(Error::InvalidCaptureConfig(...))` を返す
- `capture_ffi.rs:105` の SAFETY コメントを「Rust 側で検証済み」に修正する
- `VideoCaptureConfig` の doc コメントを更新する
- `cargo build --workspace` が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- バリデーションの単体テストを追加する（`Error::InvalidCaptureConfig` が返ること）。`tests/test_capture.rs` に配置し、`VideoCapture::new_avf()` 等の公開 API 経由でテストする（`#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]` ゲート付き）。バリデーションは FFI 呼び出しより前に `Err` を返すため、実際のカメラアクセスは発生せず、モック・スタブ不要でテスト可能
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
