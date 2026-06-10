# Linux (V4L2) で MJPEG フォーマットをパススルーで対応する

Created: 2026-06-10
Model: Opus 4.7
Polished: 2026-06-10

## なぜこの対応が必要か

USB Web カメラの多くは、**高解像度 (1080p 以上)** や **高フレームレート (30 fps 以上)** で **MJPEG (Motion JPEG, V4L2_PIX_FMT_MJPEG)** を主要フォーマットとして公開する。USB UVC 仕様上の isochronous 転送帯域制約やコスト最適化のため、カメラ側が **MJPEG のみを公開する** ケースが多い。

現在の V4L2 バックエンドは **NV12 / YUY2 / I420 の 3 フォーマットしか認識しない** ため、

- `convert_v4l2_pixel_format` (`src/video_v4l2.c:54-65`) は MJPEG を `default: 0` で除外する
- 結果、**MJPEG のみのカメラ** は `VideoDevice::formats()` で空配列になり、`video_session_create` でも開けない
- 利用者からは「**このカメラはこのライブラリでは使えない**」体験になる

この issue では **Linux V4L2 バックエンドのみ** に MJPEG をパススルー (圧縮 JPEG ペイロードをそのままユーザーコールバックに渡す) で対応する。

## 設計方針

### `mjpeg` feature による opt-in 化

MJPEG 対応は **新規 Cargo feature `mjpeg` を有効化したときのみ** 利用可能にする。

```toml
[features]
# 既存 features (変更なし)
default = ["default-v4l2", "default-avf", "default-mf"]
avf = []
mf = ["dep:windows"]
pipewire = []
v4l2 = []
default-avf = ["avf"]
default-mf = ["mf"]
default-pipewire = ["pipewire"]
default-v4l2 = ["v4l2"]
# 新規追加
mjpeg = ["v4l2"]
```

`mjpeg = ["v4l2"]` 依存により、`mjpeg` 単独有効化は不可。V4L2 バックエンドがなければ MJPEG パススルーは動作しない設計を Cargo の依存解決層で明文化する。

動機:

1. パススルーで届く **破損 JPEG の取り扱い責務** を負える利用者だけが opt-in する設計が、AGENTS.md「性能より堅牢性を優先」と整合する
2. JPEG はピクセル展開と異なり、`VideoFrame::data` のスライスの意味が変わる (NV12 等の Y プレーンではなく圧縮ストリーム) ため、明示的なスイッチを利用者に意識させる

### `build.rs` と C 側の feature 連動ガード

feature OFF 時に「公開 API および挙動が完全に変化しない」ことを保証するため、**C 側 `src/video_v4l2.c` も feature でガード** する。具体的には:

1. `build.rs` の `main()` 冒頭、`check-cfg` 群 (13-20 行) に `enable_mjpeg` と `enable_default_mjpeg` (将来の拡張用) を追記する
2. `build.rs` の **全プラットフォーム共通** の位置 (feature チェック後、target_os 分岐前) で `CARGO_FEATURE_MJPEG` が有効なら `println!("cargo::rustc-cfg=enable_mjpeg")` を発行する。これにより macOS / Windows でも `enable_mjpeg` cfg が有効になり、`capture_ffi.rs` と `capture_mf.rs` の match 網羅性が保たれる
3. `build_linux_v4l2` (`build.rs:120-130`) に `cfg!(enable_mjpeg)` 相当のチェックを追加し、`cc::Build::define("SHIGUREDO_VIDEO_DEVICE_MJPEG", None)` を呼ぶ分岐を追加する
4. `src/video_v4l2.c` の MJPEG 関連コード (`convert_v4l2_pixel_format` / `convert_video_pixel_format_to_v4l2` / `capture_thread` の MJPEG 分岐、`MJPEG_MAX_PAYLOAD_BYTES` 定数) を `#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG` で囲む
5. Rust 側の MJPEG 関連コードは `#[cfg(enable_mjpeg)]` でガードする。`enable_mjpeg` は全プラットフォームで発行されるため、macOS / Windows でも match 網羅性が保たれる
6. `src/video.h` の `VIDEO_PIXEL_FORMAT_MJPG` 定数と `FrameCallback` の MJPEG 仕様コメントは **feature ガードしない** (C ABI ヘッダとして常に提供。第三者再利用者が必要とする可能性)

これにより:

- feature OFF: C 側 `convert_v4l2_pixel_format` は MJPEG を従来どおり `default: 0` で除外する → `enumerate_device_formats` でスキップされ `formats()` に MJPEG エントリは出ない (現状と完全同一)
- feature ON: C 側で MJPEG を `VIDEO_PIXEL_FORMAT_MJPG` に変換 → Rust 側で `PixelFormat::Mjpeg` として処理

### パススルー方式

V4L2 から受け取った **JPEG ペイロードをデコードせずにそのままユーザーコールバックに渡す**。

理由:

1. AGENTS.md 「**依存は最小限にすること**」と整合する (JPEG デコーダ依存を持ち込まない)
2. AGENTS.md 「**性能より堅牢性を優先すること**」と整合する (JPEG パースのバグ・脆弱性を抱えない)
3. 利用者は **ハードウェア JPEG デコーダ** や **GPU デコード** を選べる
4. ライブラリの責務は **デバイス I/O** であって、コーデックではない

破損 JPEG (`V4L2_BUF_FLAG_ERROR` 付きフレームを含む) もそのまま通す。SOI (0xFFD8) / EOI (0xFFD9) のサニタイズは行わない。利用者は JPEG デコーダ側でエラー検出する責務を負う。`VideoFrame::data` の rustdoc にこの注意を明示する。

### `[ADD]` (後方互換のある追加) として扱う

`mjpeg` feature は default OFF のため、有効化しない既存利用者の `PixelFormat` への `match` は壊れない。C 側も feature ガードされるため、feature OFF での `formats()` 結果も従来と同一。

- `CHANGES.md` のエントリ種別: **`[ADD]`**
- ブランチ名: **`feature/add-v4l2-mjpeg-passthrough`**

### C ABI 契約の明示文書化

C コールバック `FrameCallback` (`src/video.h:36-45`) の **シグネチャは変更しない**。MJPEG の長さは `stride` 引数経由で Rust 側に伝達する。

`stride` 引数の意味が `pixel_format` に依存する状態は、C ABI を第三者再利用する利用者から見て型詐欺になり得る。**`video.h` のコメントに明示的契約として記述** する:

- NV12 / YUY2 / I420 では `stride` は **「バイト/行」**
- MJPEG では `stride` は **「JPEG ペイロード長 (バイト)」**
- 単位は `pixel_format` に依存する。**C ABI 利用者はこの契約に基づいて分岐する責務を負う**

Rust 公開 API (`VideoFrame::stride`) では MJPEG のとき **常に 0** を返し、利用者は `data.len()` で長さを得る。C ABI 層と Rust 公開 API 層で意味を分離することで、Rust 利用者には `stride` の意味を二重化しない。Rust 内部の `mjpeg_payload_bytes` ヘルパは `payload_size: i32` の引数名とし、呼び出し側コード `mjpeg_payload_bytes(stride)` の意図が分かるよう、関数定義コメントで「C ABI の `stride` 引数 (i32) を長さスロットに流用する経路」と明示する。

### 既存挙動を変えない方針 (フォールバックとサンプル)

`video_session_create` の **既定フォールバック (NV12 → YUYV)** は変更しない。MJPEG は **`mjpeg` feature 有効** かつ **`VideoCaptureConfig::pixel_format = Some(PixelFormat::Mjpeg)` で明示要求** された場合のみ選択する。

- 既定で MJPEG にフォールバックすると、`pixel_format == None` で YUV を期待していた既存利用者のコードが MJPEG 受信時に静かに壊れる
- `examples/camera_preview.rs` (raw_player は YUV のみ対応) も変更不要
- 既存実機テスト `test_capture_frames` (`tests/test_capture.rs:70`) の `assert!(frame.stride > 0)` も既定 (NV12/YUYV) で動作するため修正不要

### macOS でのエラー型統一

feature 有効時に `VideoCaptureConfig::pixel_format = Some(PixelFormat::Mjpeg)` を macOS に渡したときのエラー型を `Error::UnsupportedPixelFormat(Mjpeg)` に統一する:

- Windows: `pixel_format_to_guid(Mjpeg) = None` → 既存経路で `Error::UnsupportedPixelFormat(Mjpeg)` (`types.rs:352-358` → `capture_mf.rs` 経由)
- macOS: 現状経路では `convert_video_pixel_format_to_cv` の `default: 0` (`video_avf.m`) → C 側 NULL 返却 → `Error::SessionCreateFailed`。これを揃えるため、`capture_ffi.rs::FfiCaptureImpl::new` の `Unknown` 早期エラー (86 行) の直後に macOS 用早期エラー分岐を追加する。`cfg(enable_avf)` ガード下で行う:

```rust
// FfiCaptureImpl::new() 内、requested_pixel_format 計算の直後 (capture_ffi.rs:86 付近)
#[cfg(all(enable_mjpeg, enable_avf))]
if matches!(config.pixel_format, Some(PixelFormat::Mjpeg)) {
    return Err(Error::UnsupportedPixelFormat(PixelFormat::Mjpeg));
}
```

`VideoCapture::new_avf` (capture.rs:66-73) は `FfiCaptureImpl::new_avf` に委譲するだけであり、早期エラーは `FfiCaptureImpl::new` の入口で行うのが責務分離上正しい。

### スコープ

**対象** (feature 有効時のみ):

- V4L2 バックエンドでの MJPEG パススルーキャプチャ
- `PixelFormat::Mjpeg` バリアントとその FourCC 値の expose。Windows では `pixel_format_to_guid` が `Mjpeg => None` を返すため列挙されない非対称が生じるが、本 issue は「Linux 専用機能」として扱う
- macOS で `Some(PixelFormat::Mjpeg)` を渡したときのエラー型を `UnsupportedPixelFormat(Mjpeg)` に統一
- Windows 側 `pixel_format_to_guid` (`types.rs:352-358`) / `process_sample` (`capture_mf.rs:455-576`) の `match` 網羅維持 (バリアント追加に伴う機械的な付随変更)

**対象外 (別 issue)**:

- PipeWire の MJPEG 対応 (`SPA_VIDEO_FORMAT_ENCODED` 経由で構造が異なる)
- Windows Media Foundation での MJPEG キャプチャ実装。`guid_to_pixel_format` への `MFVideoFormat_MJPG` マップ追加は列挙とキャプチャをセットで別 issue。本 issue では `guid_to_pixel_format` は変更しない (列挙だけ Linux と揃えても Windows ではキャプチャ不能のため、`closed/0016` 流の不整合許容よりも「Linux 専用機能」と明確に切る方が API 整合性が高い)
- macOS AVFoundation での MJPEG キャプチャ実装
- V4L2 の `V4L2_PIX_FMT_JPEG` (`0x4745504A`, JFIF 限定の別 FourCC) 対応
- `PixelFormat` enum の `#[non_exhaustive]` 化 (本 issue は MJPEG 対応に専念。`#[non_exhaustive]` 化は単独で完結する `[CHANGE]` として別 issue で扱う)
- 実機検証 / MJPEG カメラの fuzzing
- MJPEG デコード API のライブラリ内提供 (依存ゼロ方針のため永続的にスコープ外)

## 現状コード (調査結果)

- `Cargo.toml:45-54` — features は 9 件。`default = ["default-v4l2", "default-avf", "default-mf"]`。`v4l2 = []` は存在
- `build.rs:77-101` — Linux セクションで `CARGO_FEATURE_V4L2` により `enable_v4l2` cfg を発行。`enable_mjpeg` は未発行
- `build.rs:120-130` — `build_linux_v4l2` は `video_v4l2.c` を `"video_v4l2"` としてコンパイル。feature 連動の `define` なし
- `src/video.h:13-15` — `VIDEO_PIXEL_FORMAT_*` 定数は NV12 / YUY2 / I420 の 3 種類のみ
- `src/video.h:36-45` — `FrameCallback` のコメント。MJPEG に関する記述なし
- `src/types.rs:11-13` — `VIDEO_PIXEL_FORMAT_*` Rust 側定数は `pub(crate)`、cfg ガードなし
- `src/types.rs:20-29` — `PixelFormat` enum は 4 バリアント (Nv12 / Yuy2 / I420 / Unknown)。`#[non_exhaustive]` 非付与
- `src/types.rs:44-51` — `to_raw` は cfg ガードなし。全プラットフォームで常にコンパイルされる
- `src/types.rs:33-41` — `from_raw` は `#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]` ガード
- `src/types.rs:54-61` — `name` は cfg ガードなし
- `src/types.rs:352-358` — Windows: `pixel_format_to_guid` は `pub(crate)`。バリアント追加時は分岐追加が必須
- `src/video_v4l2.c:54-65` — `convert_v4l2_pixel_format`: V4L2 → FourCC 変換。MJPEG 分岐なし (`default: return 0`)
- `src/video_v4l2.c:67-78` — `convert_video_pixel_format_to_v4l2`: FourCC → V4L2 変換。MJPEG 分岐なし
- `src/video_v4l2.c:102-106` — `enumerate_device_formats` は `convert_v4l2_pixel_format` が 0 を返すフォーマットをスキップ
- `src/video_v4l2.c:451-466` — `video_session_create` の既定フォールバック (NV12 → YUYV)
- `src/video_v4l2.c:574-612` — `capture_thread` は NV12 / I420 / YUYV の `if / else if / else if` 連鎖
- `src/capture_ffi.rs:86-92` — `FfiCaptureImpl::new` で `PixelFormat::Unknown(_)` は早期エラー
- `src/capture_ffi.rs:235-336` — `frame_callback` と match 分岐。`from_raw` 変換は 258 行、`Unknown(_) => return` は 332 行
- `src/capture_ffi.rs:258` — `let pf = PixelFormat::from_raw(pixel_format)` で未知フォーマットは Unknown 化
- `src/frame_math.rs:2-40` — ストライドベースのフレームサイズ計算ヘルパ (`nv12_plane_sizes` / `i420_plane_sizes` / `yuy2_packed_frame_bytes`)。MJPEG 用ヘルパなし
- `src/capture_mf.rs:455-576` — `process_sample` の `match pixel_format`。各分岐は `buffer.Unlock()` を呼んでから return / 処理
- `src/capture.rs:29-60` — `VideoCapture::new` は `#[cfg]` でバックエンド別コンストラクタに委譲する薄いラッパー
- `src/video_avf.m` — macOS `convert_video_pixel_format_to_cv` は MJPEG を `default: 0` で除外
- `src/lib.rs:17-40` — モジュール構成。`mjpeg` feature に関する記述なし

## 提案する実装

### 1. `Cargo.toml`

`[features]` セクションに `mjpeg` feature を追加 (`v4l2` 依存):

```toml
mjpeg = ["v4l2"]
```

既存の `default` や `default-*` は変更しない。

### 2. `build.rs`

`main()` の check-cfg 群 (`build.rs:13-20`) に以下を追記:

```rust
println!("cargo::rustc-check-cfg=cfg(enable_mjpeg)");
```

**全プラットフォーム共通** の位置 (feature チェック後、target_os の match 分岐の前。`build.rs:61` 付近) で MJPEG feature 用の cfg を発行:

```rust
// 全プラットフォーム共通（target_os 分岐の前）
if env::var("CARGO_FEATURE_MJPEG").is_ok() {
    println!("cargo::rustc-cfg=enable_mjpeg");
}
```

**重要な設計判断**: `enable_mjpeg` を Linux セクション内ではなく全プラットフォームで発行する理由は、`PixelFormat::Mjpeg` バリアントが `#[cfg(feature = "mjpeg")]` (OS 非依存) で存在するため、`capture_ffi.rs::frame_callback` と `capture_mf.rs::process_sample` の match 網羅性を macOS / Windows でも保つ必要があるため。

`build_linux_v4l2` (`build.rs:120-130`) に feature 連動の `define` を追加 (Linux のみ):

```rust
fn build_linux_v4l2(src_dir: &Path) {
    println!("cargo::rerun-if-changed=src/video_v4l2.c");
    println!("cargo::rerun-if-changed=src/video_v4l2.h");
    println!("cargo::rerun-if-changed=src/video.h");

    let mut build = cc::Build::new();
    build.file(src_dir.join("video_v4l2.c"));
    if std::env::var("CARGO_FEATURE_MJPEG").is_ok() {
        build.define("SHIGUREDO_VIDEO_DEVICE_MJPEG", None);
    }
    build.compile("video_v4l2");

    println!("cargo::rustc-link-lib=pthread");
}
```

### 3. `src/video.h`

定数追加 (feature ガードしない。C ABI ヘッダとして常に提供):

```c
// MJPEG は圧縮フォーマット。フレームは可変長の JPEG ペイロード。
#define VIDEO_PIXEL_FORMAT_MJPG 0x47504A4D  // 'MJPG' (Motion JPEG, V4L2 互換)
```

`FrameCallback` のコメントに **明示的契約として** 追記:

- `pixel_format` の列挙コメント (`src/video.h:27`) に `VIDEO_PIXEL_FORMAT_MJPG` を追加

- MJPEG (`VIDEO_PIXEL_FORMAT_MJPG`) の場合: `data` は JPEG ペイロード先頭、`uv_data` は NULL、**`stride` 引数は JPEG ペイロード長 (バイト)** を持つ (バイト/行ではない)、`stride_uv` は 0
- `pixel_format` ごとに `stride` の単位が異なる契約であることを明示
- `width / height` は V4L2 でネゴシエートした論理サイズ
- `pixel_buffer` は **常に NULL** (Linux 契約は `closed/0007` で確定)

### 4. `src/video_v4l2.c`

`#include` 直後に絶対上限定数を `#ifdef` ガード付きで定義:

```c
#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG
// MJPEG ペイロード長の絶対上限 (256 MiB)
// 根拠: 4K MJPEG の実フレームサイズは USB UVC で 5〜15 MB、8K MJPEG で 20〜60 MB、
// 8K HDR で 90 MB 超に達しうる。256 MiB は将来の高解像度カメラを見越した余裕値であり、
// 異常ドライバや V4L2_BUF_FLAG_ERROR 付き巨大値に対する防御線として絶対上限を設ける。
static const size_t MJPEG_MAX_PAYLOAD_BYTES = 256u * 1024u * 1024u;
#endif
```

フォーマット変換テーブルの拡張 (`src/video_v4l2.c:54-78`)。両関数の MJPEG case を `#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG` で囲む:

```c
#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG
case V4L2_PIX_FMT_MJPEG: return VIDEO_PIXEL_FORMAT_MJPG;  // convert_v4l2_pixel_format
#endif
#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG
case VIDEO_PIXEL_FORMAT_MJPG: return V4L2_PIX_FMT_MJPEG;  // convert_video_pixel_format_to_v4l2
#endif
```

`capture_thread` (`src/video_v4l2.c:574-613`) の MJPEG 分岐は、既存の `else if (session->pixel_format == V4L2_PIX_FMT_YUYV)` (603-612 行) の後ろに `#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG` ガード付きで `else if (session->pixel_format == V4L2_PIX_FMT_MJPEG)` として追加:

- `bytesused == 0` は既存ガード (567 行) でスキップ済み
- `available = min(bytesused, mmap_len)` も既存どおり
- **絶対上限ガード**: `if (available > MJPEG_MAX_PAYLOAD_BYTES) goto requeue;` を入れる。絶対上限は異常ドライバや `V4L2_BUF_FLAG_ERROR` 付き巨大 `bytesused` に対する防御線。AGENTS.md「ログはできるだけださない」方針に従い、上限超過のログ出力は行わない (サイレントスキップ)
- コールバック呼び出し: `data = mmap 先頭`, `uv_data = NULL`, `width / height = session->width / height`, `stride = (int)available`, `stride_uv = 0`, `pixel_format = VIDEO_PIXEL_FORMAT_MJPG`, `pixel_buffer = NULL`。`(int)available` キャストは、上限ガードにより `available <= MJPEG_MAX_PAYLOAD_BYTES = 256 MiB < INT_MAX` (約 2 GiB) が保証されるため安全
- SOI (0xFFD8) / EOI (0xFFD9) のサニタイズはしない (パススルー方針)
- `V4L2_BUF_FLAG_ERROR` フラグ付きフレームもそのまま通す

`video_session_create` の既定フォールバック (`src/video_v4l2.c:451-466`) は **変更しない**。

### 5. `src/types.rs`

`PixelFormat::Mjpeg` バリアントと関連を `#[cfg(feature = "mjpeg")]` でガード。

**注意**: 既存の `VIDEO_PIXEL_FORMAT_NV12` 等の定数は cfg ガードなし (11-13 行)。`to_raw` (`types.rs:44-51`) と `name` (`types.rs:54-61`) も cfg ガードなしで常にコンパイルされる。このため `VIDEO_PIXEL_FORMAT_MJPG` 定数は OS 条件を付けず `#[cfg(feature = "mjpeg")]` のみで定義する必要がある（Windows で `feature = "mjpeg"` が有効な場合も `to_raw` がコンパイルされるため）:

```rust
#[cfg(feature = "mjpeg")]
pub(crate) const VIDEO_PIXEL_FORMAT_MJPG: u32 = 0x47504A4D;

pub enum PixelFormat {
    Nv12,
    Yuy2,
    I420,
    /// MJPEG (Motion JPEG)
    ///
    /// 圧縮された JPEG フレーム。デコードは利用者の責務。
    /// `mjpeg` feature 有効時のみ存在する。
    /// V4L2 バックエンドでのみキャプチャ可能。macOS / Windows でキャプチャ要求すると
    /// `Error::UnsupportedPixelFormat(PixelFormat::Mjpeg)` を返す。
    /// `VideoFrame::data` は JPEG ペイロード、`uv_data` は `None`、
    /// `stride` / `stride_uv` は **常に 0** (意味を持たない)。
    /// **注意**: `V4L2_BUF_FLAG_ERROR` 付きフレーム (破損 JPEG) もそのまま渡る。利用者は JPEG デコーダ側で
    /// エラー検出する責務を負う。
    #[cfg(feature = "mjpeg")]
    Mjpeg,
    Unknown(u32),
}
```

変更点:

- `from_raw` (`types.rs:33-41`) の match に `#[cfg(feature = "mjpeg")] VIDEO_PIXEL_FORMAT_MJPG => PixelFormat::Mjpeg` を追加。`from_raw` は `#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]` でガード済み。`mjpeg = ["v4l2"]` 依存により `mjpeg` 有効時は `enable_v4l2` も有効のため cfg 条件は常に充足する
- `to_raw` (`types.rs:44-51`) の match に `#[cfg(feature = "mjpeg")] PixelFormat::Mjpeg => VIDEO_PIXEL_FORMAT_MJPG` を追加。`to_raw` は cfg ガードなしのため、`feature = "mjpeg"` が無効な場合 `Mjpeg` バリアントは存在せず match は網羅的
- `name` (`types.rs:54-61`) の match に `#[cfg(feature = "mjpeg")] PixelFormat::Mjpeg => "MJPEG"` を追加
- Windows 側 `pixel_format_to_guid` (`types.rs:352-358`) に **`#[cfg(feature = "mjpeg")] PixelFormat::Mjpeg => None`** を追加 (網羅性維持、必須)
- Windows 側 `guid_to_pixel_format` は **変更しない** (本 issue では Windows MJPEG 列挙対応はスコープ外)

`VideoFrame` / `VideoFrameOwned` の rustdoc 更新 (`src/types.rs:199-277` 付近):

- `data` 説明に追記:
  - 「**MJPEG の場合は圧縮された JPEG ペイロード**」
  - 「スライス寿命は他フォーマットと同じくコールバック呼び出し中のみ (`closed/0002` 契約継承)」
  - 「**MJPEG では破損 JPEG (`V4L2_BUF_FLAG_ERROR` 付きフレーム) も含まれうる**」 (太字で警告)
- `uv_data` / `stride` / `stride_uv` の各説明に「**MJPEG では未使用 (None / 0)**」を追記
- `pixel_buffer` 説明は変更不要 (Linux で NULL の契約は `closed/0007` のまま)

`VideoCaptureConfig::pixel_format` (`src/types.rs:179`) に `///` doc comment を新規追加し、`mjpeg` feature 有効時に macOS / Windows で `Some(PixelFormat::Mjpeg)` を渡すと `UnsupportedPixelFormat(Mjpeg)` で失敗する旨を明記。

### 6. `src/lib.rs`

クレートドキュメント (`src/lib.rs:12` 付近) に独立した 2 文として追記:

- `mjpeg` feature を有効化すると、V4L2 バックエンドで `V4L2_PIX_FMT_MJPEG` が `PixelFormat::Mjpeg` として既知扱いになる
- それ以外の未知 FourCC を扱う `closed/0017` の方針 (`PixelFormat::Unknown(_)` でコールバックに渡らない) は維持する

### 7. `src/capture_ffi.rs` / `src/frame_math.rs`

すべての MJPEG 関連コードを `#[cfg(enable_mjpeg)]` でガードする。

#### 7a. macOS 用早期エラー

`FfiCaptureImpl::new` (capture_ffi.rs:80-92) の `Unknown` 早期エラーの直後 (match ブロック終了直後、92 行の `};` の次の行) に追加。`cfg(enable_avf)` ガード下で行う。`enable_mjpeg` は全プラットフォームで発行されるため macOS でも有効:

```rust
// FfiCaptureImpl::new() 内 (capture_ffi.rs:86 付近)
#[cfg(all(enable_mjpeg, enable_avf))]
if matches!(config.pixel_format, Some(PixelFormat::Mjpeg)) {
    return Err(Error::UnsupportedPixelFormat(PixelFormat::Mjpeg));
}
```

#### 7b. `mjpeg_payload_bytes` ヘルパ (src/frame_math.rs)

`nv12_plane_sizes` / `i420_plane_sizes` / `yuy2_packed_frame_bytes` (2-40 行) と同じ位置に `#[cfg(enable_mjpeg)]` ガード付きで追加。`frame_callback` から呼ばれるため、`capture_ffi.rs` と同じ cfg 条件でコンパイルされる。引数型 `i32` は `frame_callback` の `stride: i32` を流用する整合性のため。関数定義コメントで「C ABI の `stride` 引数 (i32) を長さスロットに流用する経路。呼び出し側は `mjpeg_payload_bytes(stride)` の形になる」と明示する。`payload_size <= 0` を拒否し `Some(payload_size as usize)` を返す。

#### 7c. `frame_callback` の MJPEG 分岐 (capture_ffi.rs)

`match pf` (260 行の match 式。`from_raw` 呼び出しは 258 行) に `#[cfg(enable_mjpeg)]` ガード付きで新分岐を追加。既存アーム `Nv12 / I420 / Yuy2 / Unknown(_)` に対し、**`Yuy2` アーム (312-331 行) の直後、`Unknown(_)` アーム (332 行) の直前** に挿入:

```rust
#[cfg(enable_mjpeg)]
PixelFormat::Mjpeg => {
    let Some(data_size) = frame_math::mjpeg_payload_bytes(stride) else { return };
    let data_slice = unsafe { std::slice::from_raw_parts(data, data_size) };
    // MJPEG は uv_data を使用しない（圧縮ストリームであるため）
    // stride/stride_uv は 0（ピクセル行の概念が存在しないため）
    VideoFrame {
        data: data_slice,
        uv_data: None,
        width,
        height,
        stride: 0,
        stride_uv: 0,
        pixel_format: pf,
        timestamp_us,
        pixel_buffer,
    }
}
```

**注意**: `frame_callback` は全バックエンド (AVF / V4L2 / PipeWire) 共通であるため、PipeWire 経由で MJPEG が来た場合もこの分岐を通る。PipeWire MJPEG 対応は本 issue のスコープ外のため、この挙動は意図的 (パススルーが安全に動作する)。macOS は早期エラーでここに到達しない。

### 8. `src/capture_mf.rs`

`process_sample` (`capture_mf.rs:455-576`) の `match pixel_format` に `#[cfg(enable_mjpeg)]` ガード付き分岐を追加。既存アーム順 `Nv12 / I420 / Yuy2 / Unknown(_)` に対し、**`Yuy2` アームの直後、`Unknown(_)` アームの直前** に挿入。他アームと同じく `buffer.Unlock()` を呼んでから return:

```rust
#[cfg(enable_mjpeg)]
PixelFormat::Mjpeg => {
    let _ = buffer.Unlock();
    return;
}
```

`get_configured_format` 経路では `Some(PixelFormat::Mjpeg)` を受けた時点で `pixel_format_to_guid` が `None` を返し `UnsupportedPixelFormat(Mjpeg)` で弾かれるため、本分岐に実際には到達しない。網羅性維持のための保険分岐。

### 9. `CHANGES.md`

`## develop` セクションへの挿入位置は規約 (UPDATE → ADD → CHANGE → FIX) に従い、**既存の `[ADD]` の直後** に追記:

```
- [ADD] Linux (V4L2) で MJPEG パススルーに対応する mjpeg feature を追加する
  - @voluntas
```

### 10. テスト

すべての MJPEG テストは `#[cfg(feature = "mjpeg")]` ガード付きで追加する。

`tests/test_types.rs` への単体テスト追加 (計 5 件、いずれも `#[cfg(feature = "mjpeg")]` ガード)。既存テストには `name()` / `Display` の直接テストが存在しないため、これらは新規パターンとなる:

1. `PixelFormat::from_raw(0x47504A4D)` が `PixelFormat::Mjpeg` を返す
2. `PixelFormat::Mjpeg.to_raw()` が `0x47504A4D` を返す
3. `PixelFormat::Mjpeg.name()` が `"MJPEG"` を返す
4. `PixelFormat::Mjpeg` の `Display` 出力が `"MJPEG"` を返す
5. `VideoFrame::to_owned` および `VideoFrameOwned::as_frame` のラウンドトリップ MJPEG ケース。`data` には JPEG マジック (`0xFF, 0xD8, ...`) を含むダミーバイト列を入れる (人間可読のダミーデータの意図に過ぎず、`data[0..2] == [0xFF, 0xD8]` をアサート対象としない)。`stride: 0`, `stride_uv: 0`, `uv_data: None` を確認

`src/frame_math.rs::#[cfg(test)] mod tests` (74-162 行) へのヘルパ単体テスト追加 (計 2 件、いずれも `#[cfg(enable_mjpeg)]` ガード)。`mjpeg_payload_bytes` は `#[cfg(enable_mjpeg)]` ガードで定義されるため、テストも同じ cfg ガードを使用する。既存命名規約 (`nv12_rejects_non_positive_dimensions` / `nv12_small_known_sizes` 風) に揃える:

1. `mjpeg_rejects_non_positive_payload_size`: `mjpeg_payload_bytes(0)` / 負値が `None`
2. `mjpeg_payload_bytes_returns_input`: `mjpeg_payload_bytes(1024)` が `Some(1024)`

**注意**: `test_capture.rs:70` の `assert!(frame.stride > 0)` は `mjpeg` feature 有効・`PixelFormat::Mjpeg` 指定時のテスト実行では `stride` が 0 のため失敗する。本テストは実機依存かつ MJPEG カメラがなければ実行されないが、当該アサーションを `if frame.pixel_format != PixelFormat::Mjpeg { assert!(frame.stride > 0); }` のように条件付きに変更することを推奨する。

## 完了条件 (ファイル単位)

- [ ] `Cargo.toml`: `mjpeg = ["v4l2"]` feature 追加。`default` や `default-*` 変更なし
- [ ] `build.rs`: 全プラットフォーム共通の位置で `CARGO_FEATURE_MJPEG` 検出時に `println!("cargo::rustc-cfg=enable_mjpeg")` を発行、check-cfg に `enable_mjpeg` を追記。`build_linux_v4l2` で `CARGO_FEATURE_MJPEG` 検出時に `cc::Build::define("SHIGUREDO_VIDEO_DEVICE_MJPEG", None)` を呼ぶ（Linux のみ）
- [ ] `src/video.h`: `VIDEO_PIXEL_FORMAT_MJPG` 定数 (feature ガードなし)、`FrameCallback` の MJPEG 仕様 (stride 単位が pixel_format に依存する旨を含む明示的契約) コメントを追加
- [ ] `src/video_v4l2.c`: ファイル先頭に `#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG` ガード付きで `MJPEG_MAX_PAYLOAD_BYTES` (256 MiB、根拠コメント付き) を定義。`convert_v4l2_pixel_format` / `convert_video_pixel_format_to_v4l2` / `capture_thread` の MJPEG 関連コードを `#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG` で囲む。`capture_thread` の MJPEG 分岐は YUYV の `else if` の後ろに配置し、絶対上限ガードを含む
- [ ] `src/types.rs`: `#[cfg(feature = "mjpeg")]` ガード付きで `VIDEO_PIXEL_FORMAT_MJPG` 定数 (OS 条件なし)、`PixelFormat::Mjpeg` バリアント、`from_raw` / `to_raw` / `name` の match 分岐、Windows `pixel_format_to_guid` の `Mjpeg => None` を追加。`VideoFrame` / `VideoFrameOwned` / `VideoCaptureConfig` の rustdoc に MJPEG 時の解釈、`V4L2_BUF_FLAG_ERROR` 警告、macOS / Windows でのエラー型統一を明記
- [ ] `src/lib.rs`: クレートドキュメントに `mjpeg` feature 有効化時の MJPEG 既知扱いと `closed/0017` 方針維持を独立 2 文で追記
- [ ] `src/frame_math.rs`: `#[cfg(enable_mjpeg)]` ガード付きで `mjpeg_payload_bytes` ヘルパ (引数名 `payload_size: i32`、用途コメント付き) を追加。`#[cfg(test)] mod tests` に単体テスト 2 件を追加
- [ ] `src/capture_ffi.rs`: `#[cfg(all(enable_mjpeg, enable_avf))]` ガード付きで macOS 用早期エラー分岐 (`FfiCaptureImpl::new` 内) を追加、`frame_callback` の `match pf` に `#[cfg(enable_mjpeg)]` 分岐を追加
- [ ] `src/capture_mf.rs`: `process_sample` の `match` に `#[cfg(enable_mjpeg)] PixelFormat::Mjpeg => { let _ = buffer.Unlock(); return; }` を追加
- [ ] `tests/test_types.rs`: `#[cfg(feature = "mjpeg")]` ガード付き MJPEG 関連テスト 5 件を追加 (`from_raw` / `to_raw` / `name` / `Display` / VideoFrame ラウンドトリップ)
- [ ] `CHANGES.md`: `## develop` の既存 `[ADD]` の直後に新規 `[ADD]` で MJPEG feature 追加エントリを挿入
- [ ] ローカル (Linux 環境) で以下の組み合わせで `cargo build` / `cargo clippy` / `cargo test` がすべて通る。`tests/test_capture.rs` は `#[ignore]` 付きで通常実行されないため、対象は `tests/test_types.rs` の MJPEG 関連 5 件と `src/frame_math.rs` 内 `mod tests` の MJPEG 関連 2 件:
  - `cargo build` / `cargo test` (default features)
  - `cargo build --features mjpeg` / `cargo test --features mjpeg`
  - `cargo build --no-default-features --features v4l2,mjpeg`
- [ ] Linux / macOS / Windows の全プラットフォームビルドを CI で確認する (ローカルでの Windows / macOS 検証は不要)

## 影響範囲

- **`mjpeg` feature 無効時 (既定)**: `build.rs` が `SHIGUREDO_VIDEO_DEVICE_MJPEG` を define しないため C 側で MJPEG case が含まれず、Rust 側でも `PixelFormat::Mjpeg` バリアントが存在しない。公開 API および挙動は完全に変化なし。`from_raw(0x47504A4D)` は引き続き `Unknown(0x47504A4D)` を返し、MJPEG カメラは引き続き `formats()` で空 (現状と同一)
- **`mjpeg` feature 有効時 (Linux)**: MJPEG カメラを `VideoDevice::formats()` で列挙でき、`Some(PixelFormat::Mjpeg)` でキャプチャ可能。`VideoFrame::data` に JPEG ペイロード、`stride / stride_uv` は 0、`uv_data` は None
- **`mjpeg` feature 有効時 (macOS)**: `Some(PixelFormat::Mjpeg)` 指定時のみ `Error::UnsupportedPixelFormat(Mjpeg)` を返す (早期エラー分岐)。`formats()` には MJPEG は出現しない (`video_avf.m::convert_pixel_format` が MJPEG を `default: 0` で除外)
- **`mjpeg` feature 有効時 (Windows)**: `Some(PixelFormat::Mjpeg)` 指定時は `Error::UnsupportedPixelFormat(Mjpeg)` を返す (`pixel_format_to_guid` が `None`)。`formats()` には MJPEG は出現しない (`guid_to_pixel_format` を変更しないため、Windows MF MJPEG 列挙は Linux と挙動が異なるが、これは Windows MJPEG 対応 (列挙とキャプチャをセットで) を別 issue で扱うため許容)
- **C ABI**: シグネチャは変更なし。`FrameCallback` の `stride` 引数の意味が `pixel_format` に依存する契約を明示文書化する
- **既定フォールバック**: 変更なし
- **PipeWire バックエンド**: 変更なし
