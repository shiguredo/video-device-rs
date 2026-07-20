# AVF エラーパスのハードニング

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-avf-error-path-hardening
- Polished: {YYYY-MM-DD}

## 目的

macOS AVFoundation バックエンド（`src/video_avf.m`）のエラーパスに 2 つの不備がある。(1) `CVPixelBufferLockBaseAddress` の戻り値が未確認、(2) `UTF8String` が NULL を返した場合に `strdup(NULL)` で未定義動作。これらを修正する。

## 優先度根拠

- Medium。いずれもエラーパスの不備であり、通常は発生しない
- (1) はロック失敗時に NULL ポインタや不正ポインタ経由でクラッシュしうる
- (2) は `strdup(NULL)` が未定義動作（実質セグフォ）
- `/review-code` の重要指摘として確認

## 現状

### (1) CVPixelBufferLockBaseAddress の戻り値未確認（src/video_avf.m:37）

```objc
CVPixelBufferLockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
```

`CVReturn` の戻り値がチェックされていない。ロック失敗時に `CVPixelBufferGetBaseAddressOfPlane` が NULL や不正ポインタを返し、以降の `memcpy` / コールバックでクラッシュする経路がある。

### (2) UTF8String が NULL を返した場合 strdup(NULL) で未定義動作（src/video_avf.m:197-200）

```objc
const char* name = [device.localizedName UTF8String];
const char* uniqueId = [device.uniqueID UTF8String];

videoDevice->name = strdup(name);
videoDevice->unique_id = strdup(uniqueId);
```

`UTF8String` は変換不能時に NULL を返す。`strdup(NULL)` は未定義動作。

## 設計方針

### (1) の修正

`CVPixelBufferLockBaseAddress` の戻り値をチェックし、`kCVReturnSuccess` でない場合はコールバックを呼ばずに `return` する。

```objc
CVReturn lockResult = CVPixelBufferLockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
if (lockResult != kCVReturnSuccess) {
    return;
}
```

### (2) の修正

`UTF8String` の戻り値が NULL の場合、空文字列にフォールバックする。

```objc
const char* name = [device.localizedName UTF8String];
videoDevice->name = strdup(name ? name : "");
```

## 完了条件

- (1) `CVPixelBufferLockBaseAddress` の戻り値をチェックする
- (2) `UTF8String` の NULL 戻り値を防御する
- `cargo build --workspace`（macOS、default features）が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- `CHANGES.md` の `## develop` に `[FIX]` エントリを追加する

## 解決方法

{完了時に記入}
