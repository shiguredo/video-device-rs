# AVF エラーパスのハードニング

- Priority: Medium
- Created: 2026-07-20
- Completed: {YYYY-MM-DD}
- Model: qwen3.8-max-preview
- Branch: feature/fix-avf-error-path-hardening
- Polished: 2026-07-21

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

`CVReturn` の戻り値がチェックされていない。ロック失敗時に `CVPixelBufferGetBaseAddressOfPlane`（NV12: :50-51）や `CVPixelBufferGetBaseAddress`（YUY2: :62）が NULL や不正ポインタを返す。

クラッシュ経路はフォーマットにより異なる:
- **I420 パス**（:72-97）: :87-89 に実際に `memcpy` があり、NULL ソースでクラッシュする。stride=0 の場合 `calloc(1, 0)` の戻り値は実装定義で、非 NULL が返った場合 `memcpy(dst, NULL, 0)` は C 標準上 UB
- **NV12 / YUY2 パス**: `memcpy` はなくポインタをコールバックに渡すだけ。Rust 側 `capture_ffi.rs:263` で `data.is_null()` チェックがあり NULL なら早期 return するが、NULL でない不正ポインタを `from_raw_parts` で参照した場合は UB

ロック失敗時に `CVPixelBufferGetBytesPerRowOfPlane`（:52-53, :75-77）が 0 を返す可能性もある。NV12/YUY2 では Rust 側 `frame_math` が `stride <= 0` で `None` を返し早期 return するためクラッシュしない。

### (2) UTF8String が NULL を返した場合 strdup(NULL) で未定義動作（src/video_avf.m:197-201）

```objc
const char* name = [device.localizedName UTF8String];
const char* uniqueId = [device.uniqueID UTF8String];

videoDevice->name = strdup(name);
videoDevice->unique_id = strdup(uniqueId);
```

`UTF8String` はレシーバが nil の場合に NULL を返す（ObjC の nil メッセージングで 0 が返る）。`localizedName` / `uniqueID` は Apple のドキュメント上 non-optional であり、有効な `AVCaptureDevice` に対して nil になることは現実的にほぼないが、ObjC のランタイムレベルでは nil メッセージングで NULL が返りうる。`strdup(NULL)` は未定義動作。

## 設計方針

### (1) の修正

`CVPixelBufferLockBaseAddress` の戻り値をチェックし、`kCVReturnSuccess` でない場合はコールバックを呼ばずに `return` する。ロック失敗時はバッファがロックされていないため、`CVPixelBufferUnlockBaseAddress`（:100）は呼ばなくてよい。

```objc
CVReturn lockResult = CVPixelBufferLockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
if (lockResult != kCVReturnSuccess) {
    NSLog(@"CVPixelBufferLockBaseAddress failed: %d", lockResult);
    return;
}
```

後方互換: これまでクラッシュしていた経路がフレームドロップ（コールバックを呼ばずに return）に変わる。挙動変更であり改善。CHANGES.md の `[FIX]` エントリに含める。

ログ出力: ロック失敗は通常発生しない異常系であり、デバッグ時に検知できるよう `NSLog(@"CVPixelBufferLockBaseAddress failed: %d", lockResult);` を出す（AGENTS.md 規約によりログメッセージは英語）。

### (2) の修正

`UTF8String` の戻り値が NULL の場合、空文字列にフォールバックする。空文字列の `name` / `unique_id` を持つデバイスも列挙に含める（スキップしない）。`localizedName` / `uniqueID` が nil になることは現実的にほぼなく、空文字列での列挙は防御的フォールバックとして十分。空の `unique_id` でセッション作成を試みた場合、`video_avf_session_create`（:348-350）の `deviceWithUniqueID:` が nil を返し、:353-355 のデフォルトデバイスフォールバックに遷移する。クラッシュにはならず、実害は意図しないデバイスが選ばれる可能性に留まる。

```objc
const char* name = [device.localizedName UTF8String];
const char* uniqueId = [device.uniqueID UTF8String];

videoDevice->name = strdup(name ? name : "");
videoDevice->unique_id = strdup(uniqueId ? uniqueId : "");
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
