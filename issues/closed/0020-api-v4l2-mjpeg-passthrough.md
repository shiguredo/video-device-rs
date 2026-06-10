# Linux (V4L2) で MJPEG フォーマットをパススルーで対応する

Created: 2026-06-10
Model: Opus 4.7
Polished: 2026-06-10

## なぜこの対応が必要か

USB Web カメラの多くは、**高解像度 (1080p 以上)** や **高フレームレート (30 fps 以上)** で **MJPEG (Motion JPEG, V4L2_PIX_FMT_MJPEG)** を主要フォーマットとして公開する。カメラ側が **MJPEG のみを公開する** ケースが多い。

現在の V4L2 バックエンドは **NV12 / YUY2 / I420 の 3 フォーマットしか認識しない** ため、

- `convert_v4l2_pixel_format` (`src/video_v4l2.c:54-65`) は MJPEG を `default: 0` で除外する
- 結果、**MJPEG のみのカメラ** は `VideoDevice::formats()` で空配列になり、`video_session_create` でも開けない
- 利用者からは「**このカメラはこのライブラリでは使えない**」体験になる

この issue では **Linux V4L2 バックエンドのみ** に MJPEG をパススルー (圧縮 JPEG ペイロードをそのままユーザーコールバックに渡す) で対応する。

## 設計方針

### `PixelFormat::Mjpeg` は常に定義する

`PixelFormat::Mjpeg` バリアントを feature フラグで増減させると、全プラットフォームの match 式に `#[cfg(feature = "mjpeg")]` ガードが必要になり、保守が極めて困難になる。このため、**`PixelFormat::Mjpeg` は `mjpeg` feature の有無に関わらず常に定義する**。

`mjpeg` feature は **キャプチャ実装側の可否** のみを制御する:

- `mjpeg` feature 有効かつ V4L2 環境: MJPEG キャプチャが動作する
- それ以外の環境で `PixelFormat::Mjpeg` を指定した場合: 実行時エラー (`Error::UnsupportedPixelFormat(PixelFormat::Mjpeg)`) を返す

動機: JPEG は圧縮ストリームであり `VideoFrame::data` の意味がピクセルフォーマットと異なるため、破損 JPEG の取り扱い含めて利用者に明示的な opt-in を求める必要がある。これは AGENTS.md「性能より堅牢性を優先」と整合する。

### `mjpeg` feature の定義

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

### `build.rs` と C 側の feature 連動ガード

feature OFF 時に C 側で MJPEG がフィルタされることを保証するため、**C 側 `src/video_v4l2.c` も feature でガード** する。具体的には:

1. `build.rs` の `main()` 冒頭、`check-cfg` 群 (13-20 行) に `println!("cargo::rustc-check-cfg=cfg(enable_mjpeg)");` を追記する
2. `build.rs` の Linux `"linux"` アーム内で `CARGO_FEATURE_MJPEG` が有効なら `println!("cargo::rustc-cfg=enable_mjpeg")` を発行する (V4L2 cfg 発行の直後)
3. `build_linux_v4l2` (`build.rs:120-130`) に `CARGO_FEATURE_MJPEG` 検出時の分岐を追加し、`cc::Build::define("SHIGUREDO_VIDEO_DEVICE_MJPEG", "1")` を呼ぶ。`"1"` を明示的に渡すのは、`None` で値なしマクロを生成すると libclang の AST 解析で誤検出される可能性があるため。`"1"` により明示的な有効化フラグとして扱う
4. `src/video_v4l2.c` の MJPEG 関連コード (`convert_v4l2_pixel_format` / `convert_video_pixel_format_to_v4l2` / `capture_thread` の MJPEG 分岐、`MJPEG_MAX_PAYLOAD_BYTES` 定数) を `#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG` で囲む
5. Rust 側のキャプチャ実装内の MJPEG 分岐は `#[cfg(enable_mjpeg)]` で実際のパススルー処理をガードし、`#[cfg(not(enable_mjpeg))]` 側は `return` (フレームコールバック) または `Err(Error::UnsupportedPixelFormat(...))` (キャプチャ初期化) を返す
6. `src/video.h` の `VIDEO_PIXEL_FORMAT_MJPG` 定数と `FrameCallback` の MJPEG 仕様コメントは **feature ガードしない** (C ABI ヘッダとして常に提供)

### パススルー方式

V4L2 から受け取った **JPEG ペイロードをデコードせずにそのままユーザーコールバックに渡す**。

理由:

1. AGENTS.md 「**依存は最小限にすること**」と整合する (JPEG デコーダ依存を持ち込まない)
2. AGENTS.md 「**性能より堅牢性を優先すること**」と整合する (JPEG パースのバグ・脆弱性を抱えない)
3. ライブラリの責務は **デバイス I/O** であって、コーデックではない (利用者はハードウェア JPEG デコーダや GPU デコードを自由に選べる)

破損 JPEG (`V4L2_BUF_FLAG_ERROR` 付きフレームを含む) もそのまま通す。SOI (0xFFD8) / EOI (0xFFD9) のサニタイズは行わない。利用者は JPEG デコーダ側でエラー検出する責務を負う。注意: `V4L2_BUF_FLAG_ERROR` の未検査は既存の全フォーマット (NV12 / I420 / YUYV) に共通する挙動であり、MJPEG に限定されない。`VideoFrame::data` の rustdoc にはこの注意を全フォーマット横断の警告として追記する。

### `[CHANGE]` (後方互換のない変更) として扱う

`PixelFormat` enum に `Mjpeg` バリアントを追加すると、`pub enum` に `#[non_exhaustive]` が付与されていないため、`PixelFormat` に対して exhaustive match を書いている外部コードがコンパイルエラーになる。よって本変更は `[CHANGE]` (後方互換のない変更) として扱う。

- `CHANGES.md` のエントリ種別: **`[CHANGE]`**
- ブランチ名: **`feature/change-v4l2-mjpeg-passthrough`**
- 移行ガイド: `CHANGES.md` の `[CHANGE]` エントリで `Mjpeg` アーム追加の必要性を説明する。`PixelFormat` の doc comment にも exhaustive match 破壊の注意を追記する。将来的な `#[non_exhaustive]` 化は別 issue で扱う

### C ABI 契約の明示文書化

C コールバック `FrameCallback` (`src/video.h:36-45`) の **シグネチャは変更しない**。MJPEG の長さは `stride` 引数経由で Rust 側に伝達する。

`stride` 引数の意味が `pixel_format` に依存する状態は、C ABI を第三者再利用する利用者から見て型詐欺になり得る。**`video.h` のコメントに明示的契約として記述** する:

- NV12 / YUY2 / I420 では `stride` は **「バイト/行」**
- MJPEG では `stride` は **「JPEG ペイロード長 (バイト)」**
- 単位は `pixel_format` に依存する。**C ABI 利用者はこの契約に基づいて分岐する責務を負う**
- 注意: `stride` 引数の意味二重化は可変長フォーマット（MJPEG / JPEG / H.264 圧縮ストリーム等）のパススルー追加が増えるほど C ABI 利用者の分岐負担が増大する設計上の制約である。根本対策は C ABI の変更が必要だが、後方互換性のため本 issue では現行シグネチャを維持する

Rust 公開 API (`VideoFrame::stride`) では MJPEG のとき **常に 0** を返し、利用者は `data.len()` で JPEG ペイロード長を得る。C ABI 層と Rust 公開 API 層で意味を分離することで、Rust 利用者には `stride` の意味を二重化しない。`VideoFrame::data` の rustdoc に「MJPEG の場合は通常の行ストライドではなく JPEG 圧縮データが格納される。長さは `data.len()` で取得すること」と明記する。Rust 内部の `mjpeg_payload_bytes` ヘルパは `payload_size: i32` の引数名とし、呼び出し側コード `mjpeg_payload_bytes(stride)` の意図が分かるよう、関数定義コメントで「C ABI の `stride` 引数 (i32) を長さスロットに流用する経路」と明示する。

### 既存挙動を変えない方針 (フォールバックとサンプル)

`video_session_create` の **既定フォールバック (NV12 → YUYV)** は変更しない。MJPEG は **`mjpeg` feature 有効** かつ **`VideoCaptureConfig::pixel_format = Some(PixelFormat::Mjpeg)` で明示要求** された場合のみ選択する。

- 既定で MJPEG にフォールバックすると、`pixel_format == None` で YUV を期待していた既存利用者のコードが MJPEG 受信時に静かに壊れる (例: `frame.data` を Y プレーンと仮定して画素処理を行うコードが JPEG 圧縮データを受け取る)
- `examples/camera_preview.rs` の raw_player コアロジックは既定フォールバックのため対象外 (MJPEG 用 match アームは `enqueue_video_frame` に本 issue で追加する)

### 実行時エラーの統一

`PixelFormat::Mjpeg` が常に定義されるため、`mjpeg` feature 非有効時や非 V4L2 プラットフォームで `PixelFormat::Mjpeg` をキャプチャ要求した場合のエラー型を `Error::UnsupportedPixelFormat(PixelFormat::Mjpeg)` に統一する。**すべてのバックエンドで**同一のエラー型を返す。

- Rust 側 `FfiCaptureImpl::new` で `#[cfg(not(enable_mjpeg))]` ガード下の早期エラーチェック
- `FfiCaptureImpl::new_pipewire` で MJPEG を常に拒否 (PipeWire は MJPEG 非対応)
- Windows 側は `pixel_format_to_guid(Mjpeg) = None` により `capture_mf.rs` の既存経路でも `UnsupportedPixelFormat(Mjpeg)` が返る

### スコープ

**対象**:

- V4L2 バックエンドでの MJPEG パススルーキャプチャ (feature 有効時のみ)
- `PixelFormat::Mjpeg` バリアントとその FourCC 値の常時定義 (feature 非依存)
- 全プラットフォームでの match 網羅性確保と実行時エラー統一
- `VideoFrame` / `VideoFrameOwned` / `VideoCaptureConfig` の rustdoc に MJPEG 仕様を明記

**対象外 (別 issue)**:

- PipeWire の MJPEG 対応 (`SPA_VIDEO_FORMAT_ENCODED` 経由で構造が異なる)
- Windows Media Foundation での MJPEG キャプチャ実装。`guid_to_pixel_format` への `MFVideoFormat_MJPG` マップ追加は列挙とキャプチャをセットで別 issue
- macOS AVFoundation での MJPEG キャプチャ実装
- V4L2 の `V4L2_PIX_FMT_JPEG` (`0x4745504A`) 対応。Video4Linux 仕様上、`V4L2_PIX_FMT_MJPEG` (`0x47504A4D`) と `V4L2_PIX_FMT_JPEG` (`0x4745504A`) は別の FourCC として定義されている。`MJPEG` は UVC Motion JPEG、`JPEG` は JFIF 準拠 JPEG の意味論だが、実際のデバイス実装ではどちらを使うかベンダー依存。本 issue では主要な MJPEG に対応し、JPEG の対応は別 issue で実施する（追加の判別ロジックとテストが必要なためスコープを分離する）
- `PixelFormat` enum の `#[non_exhaustive]` 化 (本 issue は MJPEG 対応に専念。`#[non_exhaustive]` 化は単独で完結する `[CHANGE]` として別 issue で扱う)
- 実機検証 / MJPEG カメラの fuzzing
- MJPEG デコード API のライブラリ内提供 (依存ゼロ方針のため永続的にスコープ外)

## 現状コード (調査結果)

- `Cargo.toml:45-54` — features は 9 件。`default = ["default-v4l2", "default-avf", "default-mf"]`。`v4l2 = []` は存在
- `build.rs:34-36` — Linux セクションで `CARGO_FEATURE_V4L2` により `enable_v4l2` cfg を発行。`enable_mjpeg` は未発行
- `build.rs:120-130` — `build_linux_v4l2` は `video_v4l2.c` を `"video_v4l2"` としてコンパイル。feature 連動の `define` なし
- `src/video.h:13-15` — `VIDEO_PIXEL_FORMAT_*` 定数は NV12 / YUY2 / I420 の 3 種類のみ
- `src/video.h:36-45` — `FrameCallback` のコメント。MJPEG に関する記述なし
- `src/types.rs:11-13` — `VIDEO_PIXEL_FORMAT_*` Rust 側定数は `pub(crate)`、cfg ガードなし
- `src/types.rs:20-29` — `PixelFormat` enum は 4 バリアント (Nv12 / Yuy2 / I420 / Unknown)。`#[non_exhaustive]` 非付与
- `src/types.rs:44-51` — `to_raw` は cfg ガードなし。全プラットフォームで常にコンパイルされる
- `src/types.rs:33-41` — `from_raw` は `#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]` ガード
- `src/types.rs:54-61` — `name` は cfg ガードなし
- `src/types.rs:352-358` — Windows: `pixel_format_to_guid` は `pub(crate)`
- `src/video_v4l2.c:54-65` — `convert_v4l2_pixel_format`: V4L2 → FourCC 変換。MJPEG 分岐なし (`default: return 0`)
- `src/video_v4l2.c:67-78` — `convert_video_pixel_format_to_v4l2`: FourCC → V4L2 変換。MJPEG 分岐なし
- `src/video_v4l2.c:102-106` — `enumerate_device_formats` は `convert_v4l2_pixel_format` が 0 を返すフォーマットをスキップ
- `src/video_v4l2.c:451-466` — `video_session_create` の既定フォールバック (NV12 → YUYV)
- `src/video_v4l2.c:574-612` — `capture_thread` は NV12 / I420 / YUYV の `if / else if / else if` 連鎖。612 行目の `}` は YUYV の `else if` 閉じ括弧
- `src/capture_ffi.rs:86-92` — `FfiCaptureImpl::new` で `PixelFormat::Unknown(_)` は早期エラー
- `src/capture_ffi.rs:235-336` — `frame_callback` と match 分岐。`from_raw` 変換は 258 行、`Unknown(_) => return` は 332 行
- `src/capture_ffi.rs:258` — `let pf = PixelFormat::from_raw(pixel_format)` で未知フォーマットは Unknown 化
- `src/frame_math.rs:2-40` — ストライドベースのフレームサイズ計算ヘルパ (`nv12_plane_sizes` / `i420_plane_sizes` / `yuy2_packed_frame_bytes`)。MJPEG 用ヘルパなし
- `src/capture_mf.rs:455-576` — `process_sample` の `match pixel_format`。`_buf_guard` (486 行) 経由で Lock/Unlock が管理され、各フォーマット分岐でフレーム処理
- `src/capture.rs:29-60` — `VideoCapture::new` は `#[cfg]` でバックエンド別コンストラクタに委譲する薄いラッパー
- `src/video_avf.m` — macOS `convert_video_pixel_format_to_cv` は MJPEG を `default: 0` で除外
- `src/lib.rs:17-40` — モジュール構成。`mjpeg` feature に関する記述なし

## 提案する実装

### 1. `Cargo.toml`

`[features]` セクションに `mjpeg` feature を追加 (`v4l2` 依存):

```toml
mjpeg = ["v4l2"]
```

`[dev-dependencies]` に `proptest` を追加 (AGENTS.md「PBT は proptest を使うこと」に従う):

```toml
[dev-dependencies]
proptest = "1"
```

`Cargo.toml` 末尾に `[[test]]` セクションを追加 (`pbt/tests/prop_types.rs` をテストとして認識させる):

```toml
[[test]]
name = "prop_types"
path = "pbt/tests/prop_types.rs"
```

既存の `default` や `default-*` は変更しない。

### 2. `build.rs`

`main()` の check-cfg 群 (`build.rs:13-20`) に以下を追記:

```rust
println!("cargo::rustc-check-cfg=cfg(enable_mjpeg)");
```

**Linux** の `"linux"` アーム内 (`build.rs:33-47`)、`CARGO_FEATURE_V4L2` の cfg 発行 (34-36 行) の直後に MJPEG feature 用の cfg を発行:

```rust
"linux" => {
    // ... 既存の v4l2 チェック ...
    if env::var("CARGO_FEATURE_MJPEG").is_ok() {
        println!("cargo::rustc-cfg=enable_mjpeg");
    }
    // ... 既存の pipewire チェック ...
}
```

`mjpeg = ["v4l2"]` 依存により `CARGO_FEATURE_MJPEG` が有効なら `CARGO_FEATURE_V4L2` も必ず有効であるため、`enable_v4l2` と `enable_mjpeg` の両方が発行される。ただし、`build.rs` の cfg 発行は Linux `"linux"` アーム内のみで行うため、macOS / Windows で `--features mjpeg` を指定しても `enable_mjpeg` は発行されず、Rust 側の `#[cfg(not(enable_mjpeg))]` チェックが機能する。MJPEG パススルーは Linux V4L2 でのみ動作する。

`build_linux_v4l2` (`build.rs:120-130`) に feature 連動の `define` を追加 (Linux のみ):

```rust
fn build_linux_v4l2(src_dir: &Path) {
    println!("cargo::rerun-if-changed=src/video_v4l2.c");
    println!("cargo::rerun-if-changed=src/video_v4l2.h");
    println!("cargo::rerun-if-changed=src/video.h");

    let mut build = cc::Build::new();
    build.file(src_dir.join("video_v4l2.c"));
    if std::env::var("CARGO_FEATURE_MJPEG").is_ok() {
        build.define("SHIGUREDO_VIDEO_DEVICE_MJPEG", "1");
    }
    build.compile("video_v4l2");

    println!("cargo::rustc-link-lib=pthread");
}
```

`let mut build` による可変変数パターンは、条件付き `define` 追加のため builder チェーンでは書けないためやむを得ない。

### 3. `src/video.h`

定数追加 (feature ガードしない。C ABI ヘッダとして常に提供):

```c
// MJPEG は圧縮フォーマット。フレームは可変長の JPEG ペイロード。
#define VIDEO_PIXEL_FORMAT_MJPG 0x47504A4D  // 'MJPG' (Motion JPEG, V4L2 互換)
```

`FrameCallback` のコメントに **明示的契約として** 追記:

- `pixel_format` の列挙コメント (`src/video.h:27`) を `VIDEO_PIXEL_FORMAT_NV12 / VIDEO_PIXEL_FORMAT_YUY2 / VIDEO_PIXEL_FORMAT_I420 / VIDEO_PIXEL_FORMAT_MJPG` に変更
- MJPEG (`VIDEO_PIXEL_FORMAT_MJPG`) の場合: `data` は JPEG ペイロード先頭、`uv_data` は NULL、**`stride` 引数は JPEG ペイロード長 (バイト)** (バイト/行ではない)、`stride_uv` は 0
- `pixel_format` ごとに `stride` の単位が異なる契約であることを明示
- `width / height` は V4L2 でネゴシエートした論理サイズ
- `pixel_buffer` は **常に NULL** (Linux 契約は `closed/0007` で確定)

### 4. `src/video_v4l2.c`

`#include` 直後に絶対上限定数を `#ifdef` ガード付きで定義:

```c
#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG
// MJPEG ペイロード長の絶対上限 (256 MiB)
// 根拠: UVC 1.5 仕様のアイソクロナス転送最大ペイロードは High Speed (480 Mbps) で 3072 バイト/マイクロフレーム、
// SuperSpeed (5 Gbps) で 1024 バイト/マイクロフレーム。
// 実フレームサイズは 4K MJPEG で 5〜15 MiB、8K MJPEG で 20〜60 MiB、8K HDR で 90 MiB 超に達しうる。
// V4L2 の `bytesused` は `u32` で最大 4 GiB だが、256 MiB は以下の理由で選択:
// 1. 将来の 8K や 16K 高フレームレートカメラを見越した十分な余裕値
// 2. `int` (32-bit signed) でサイズを扱う既存コードパスとの互換性 (256 MiB < INT32_MAX)
// 3. malloc/stack 割り当てにおける現実的な上限として過度に大きくない
// 異常ドライバや V4L2_BUF_FLAG_ERROR 付き巨大値に対する防御線として機能する。
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

`capture_thread` (`src/video_v4l2.c`、YUYV の `else if` ブロック直後。新規定数追加で行番号は 2 行ずれるため絶対行番号ではなく YUYV `else if` 閉じ括弧の直後という相対位置で考える) の MJPEG 分岐を以下の手順で追加する:

```c
#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG
            } else if (session->pixel_format == V4L2_PIX_FMT_MJPEG) {
                if (available > MJPEG_MAX_PAYLOAD_BYTES) {
                    goto requeue;
                }
                session->callback(session->user_data, data, NULL,
                                  session->width, session->height,
                                  (int)available, 0,
                                  VIDEO_PIXEL_FORMAT_MJPG, timestamp_us, NULL);
            }
#else
            }
#endif
```

補足:
- `#ifdef` 有効時: `} else if (MJPEG) { ... }` が現れ、YUYV ブロックを閉じつつ MJPEG ブロックを開始する
- `#ifdef` 非有効時: `#else` 節の `}` のみが残り、YUYV ブロックを閉じる。if/else if チェーンが構文破綻しない
- `goto requeue` による MJPEG_MAX_PAYLOAD_BYTES (256 MiB) 超過時のサイレント破棄は、既存全フォーマットのエラーフレームスキップと一貫する

### 5. `src/types.rs`

#### 5a. 定数追加

```rust
// MJPEG (Motion JPEG)
pub(crate) const VIDEO_PIXEL_FORMAT_MJPG: u32 = 0x47504A4D;
```

#### 5b. `PixelFormat` 列挙型の拡張

`PixelFormat::Mjpeg` バリアントを常に定義する (cfg ガードを一切付けない)。

`PixelFormat` enum の doc comment (`src/types.rs:15-18`) を以下の形に修正する（既存 doc の不正確さ — `from_raw` が Windows で利用できないにも関わらず「全プラットフォームで利用可能」と記述されている — も同時に修正する。この修正は `[CHANGE]` のリリースノートに含めるが、doc 修正単体としての `[FIX]` エントリは不要）:

```rust
/// ピクセルフォーマット
///
/// 列挙・キャプチャは Media Foundation の `GUID` と内部で対応付けている。
/// `to_raw` は全プラットフォームで利用可能。
/// `from_raw` は FFI バックエンド有効時 (AVF / V4L2 / PipeWire) に利用可能。
/// それ以外 (Windows/mf) ではコンパイルエラーになる。
```

```rust
pub enum PixelFormat {
    Nv12,
    Yuy2,
    I420,
    /// MJPEG (Motion JPEG)
    ///
    /// 圧縮された JPEG フレーム。デコードは利用者の責務。
    /// V4L2 バックエンド + `mjpeg` feature 有効時のみキャプチャ可能。
    /// それ以外の環境でキャプチャ要求すると `Error::UnsupportedPixelFormat(PixelFormat::Mjpeg)` を返す。
    /// `VideoFrame::data` は JPEG ペイロード、`uv_data` は `None`、
    /// `stride` / `stride_uv` は **常に 0** (意味を持たない)。
    /// **注意**: `V4L2_BUF_FLAG_ERROR` 付きフレーム (破損 JPEG) もそのまま渡る。利用者は JPEG デコーダ側で
    /// エラー検出する責務を負う。
    Mjpeg,
    Unknown(u32),
}
```

#### 5c. `from_raw` の拡張

`from_raw` (`types.rs:33-41`) の match に `VIDEO_PIXEL_FORMAT_MJPG => PixelFormat::Mjpeg` を追加。`from_raw` は `#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]` でガード済みだが、`VIDEO_PIXEL_FORMAT_MJPG` 定数自体は cfg ガードなしで定義されるため、cfg 条件を追加する必要はない。Mjpeg 分岐は既存の `_ => PixelFormat::Unknown(raw)` の直前に配置し、`from_raw` がコンパイルされる全プラットフォームで一致する挙動とする:

```rust
VIDEO_PIXEL_FORMAT_MJPG => PixelFormat::Mjpeg,
```

#### 5d. `to_raw` の拡張

`to_raw` (`types.rs:44-51`) の match に `PixelFormat::Mjpeg => VIDEO_PIXEL_FORMAT_MJPG` を追加。`to_raw` は cfg ガードなし、`Mjpeg` バリアントも常に存在するため、cfg 条件は不要:

```rust
PixelFormat::Mjpeg => VIDEO_PIXEL_FORMAT_MJPG,
```

#### 5e. `name` の拡張

`name` (`types.rs:54-61`) の match に `PixelFormat::Mjpeg => "MJPEG"` を追加:

```rust
PixelFormat::Mjpeg => "MJPEG",
```

#### 5f. `Display` の拡張

`Display` (`types.rs:64-68`) の match に `PixelFormat::Mjpeg => write!(f, "MJPEG")` を追加:

```rust
PixelFormat::Mjpeg => write!(f, "MJPEG"),
```

#### 5g. `pixel_format_to_guid` の拡張 (Windows のみ)

Windows 側 `pixel_format_to_guid` (`types.rs:352-358`) に `PixelFormat::Mjpeg => None` を追加。`Mjpeg` バリアントは常に存在するため、cfg 条件は不要。これにより Windows で `PixelFormat::Mjpeg` を指定した場合、`capture_mf.rs` の既存経路で `Error::UnsupportedPixelFormat(PixelFormat::Mjpeg)` が返る:

```rust
PixelFormat::Mjpeg => None,
```

#### 5h. `guid_to_pixel_format` は変更しない

Windows 側 `guid_to_pixel_format` は **変更しない** (本 issue では Windows MJPEG 列挙対応はスコープ外)。

#### 5i. `VideoFrame` / `VideoFrameOwned` の rustdoc 更新

`VideoFrame` / `VideoFrameOwned` の rustdoc 更新 (`src/types.rs:199-277` 付近):

- `data` 説明に追記:
  - 「**MJPEG の場合は圧縮された JPEG ペイロード**」
  - 「スライス寿命は他フォーマットと同じくコールバック呼び出し中のみ (`closed/0002` 契約継承)」
  - 「**MJPEG では破損 JPEG (`V4L2_BUF_FLAG_ERROR` 付きフレーム) も含まれうる**」 (太字で警告)
  - 「`V4L2_BUF_FLAG_ERROR` が立ったフレームかどうかは本 API では判別できない。利用者は JPEG デコーダのエラーから間接的に判断すること」
- `uv_data` の説明に「**MJPEG では None**」を追記
- `stride` の説明に「**MJPEG では 0**」を追記 (`VideoFrame` と `VideoFrameOwned` の両方)
- `stride_uv` の説明に「**MJPEG では 0**」を追記
- `pixel_buffer` 説明は変更不要 (Linux で NULL の契約は `closed/0007` のまま)

#### 5j. `VideoCaptureConfig::pixel_format` の rustdoc 更新

`VideoCaptureConfig::pixel_format` (`src/types.rs:179`) に `///` doc comment を新規追加し、`mjpeg` feature の有無に関わらず非 V4L2 環境 (macOS / Windows) で `Some(PixelFormat::Mjpeg)` を渡すと `Error::UnsupportedPixelFormat(PixelFormat::Mjpeg)` で失敗する旨を明記。

### 6. `src/lib.rs`

クレートドキュメント (`src/lib.rs:12` 付近) に独立した 2 文として追記:

- `mjpeg` feature を有効化すると、V4L2 バックエンドで `V4L2_PIX_FMT_MJPEG` が `PixelFormat::Mjpeg` として既知扱いになる
- それ以外の未知 FourCC を扱う `closed/0017` の方針 (`PixelFormat::Unknown(_)` でコールバックに渡らない) は維持する

### 7. `src/capture_ffi.rs` / `src/frame_math.rs`

#### 7a. 実行時エラーチェック

`FfiCaptureImpl::new` (capture_ffi.rs:80-92) の `Unknown` 早期エラーの直後 (match ブロック終了直後、92 行の `};` の次の行) に追加:

```rust
// MJPEG キャプチャは enable_mjpeg 環境 (Linux V4L2 + mjpeg feature) でのみ対応。
// enable_mjpeg が真なら enable_v4l2 も必ず真であるため、単独チェックで十分。
#[cfg(not(enable_mjpeg))]
if matches!(config.pixel_format, Some(PixelFormat::Mjpeg)) {
    return Err(Error::UnsupportedPixelFormat(PixelFormat::Mjpeg));
}
```

このチェックでカバーされるケース:

- macOS (AVF) + mjpeg feature 有効/非有効: エラー (`enable_v4l2` は Linux でのみ発行)
- Windows (MF) + mjpeg feature 有効/非有効: エラー (同上)

PipeWire バックエンドは `new_pipewire()` で個別にチェックする (後述)。Linux + mjpeg 環境では `#[cfg(not(enable_mjpeg))]` のチェックがコンパイル除去されるが、PipeWire は `new_pipewire()` 側で拒否するため `UnsupportedPixelFormat(Mjpeg)` で統一される。

`FfiCaptureImpl::new_pipewire` (`capture_ffi.rs:149-156`) の先頭に MJPEG 拒否を追加:

```rust
#[cfg(enable_pipewire)]
pub(crate) fn new_pipewire<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
where
    F: Fn(VideoFrame<'_>) + Send + 'static,
{
    // PipeWire は MJPEG 非対応。feature フラグに関わらず拒否する
    if matches!(config.pixel_format, Some(PixelFormat::Mjpeg)) {
        return Err(Error::UnsupportedPixelFormat(PixelFormat::Mjpeg));
    }
    Self::new(&OPS_PIPEWIRE, config, callback)
}
```

これにより全プラットフォーム・全バックエンドで `PixelFormat::Mjpeg` 指定時のエラー型が `UnsupportedPixelFormat(Mjpeg)` に統一される。

#### 7b. `mjpeg_payload_bytes` ヘルパ (src/frame_math.rs)

`nv12_plane_sizes` / `i420_plane_sizes` / `yuy2_packed_frame_bytes` (2-40 行) と同じ位置に `#[cfg(enable_mjpeg)]` ガード付きで追加。`frame_callback` から呼ばれるため、`capture_ffi.rs` と同じ cfg 条件でコンパイルされる。引数型 `i32` は `frame_callback` の `stride: i32` を流用する整合性のため。

```rust
/// C ABI の `stride` 引数 (i32) を JPEG ペイロード長スロットとして流用する経路。
/// 呼び出し側は `mjpeg_payload_bytes(stride)` の形で呼ぶ。
/// `payload_size <= 0` の場合は `None` を返す。
#[cfg(enable_mjpeg)]
pub(crate) fn mjpeg_payload_bytes(payload_size: i32) -> Option<usize> {
    if payload_size <= 0 {
        return None;
    }
    Some(payload_size as usize)
}
```

#### 7c. `frame_callback` の MJPEG 分岐 (capture_ffi.rs)

`match pf` (260 行の match 式。`from_raw` 呼び出しは 258 行) に新分岐を追加。既存アーム `Nv12 / I420 / Yuy2 / Unknown(_)` に対し、**`Yuy2` アーム (312-331 行) の直後、`Unknown(_)` アーム (332 行) の直前** に挿入。**cfg ガードは付けず、分岐内部を cfg で分ける**:

```rust
PixelFormat::Mjpeg => {
    #[cfg(enable_mjpeg)]
    {
        let Some(data_size) = frame_math::mjpeg_payload_bytes(stride) else { return };
        let data_slice = unsafe { std::slice::from_raw_parts(data, data_size) };
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
    #[cfg(not(enable_mjpeg))]
    {
        // MJPEG 非対応環境では C 側で MJPEG がフィルタされるため到達しない
        return;
    }
}
```

**注意**: `frame_callback` は全バックエンド (AVF / V4L2 / PipeWire) 共通であるため、PipeWire 経由で MJPEG が来た場合もこの分岐を通る。PipeWire MJPEG 対応は本 issue のスコープ外だが、非対応環境では C 側でフィルタされるか、Rust 側の初期化時エラーでここに到達しない。

### 8. `src/capture_mf.rs`

`process_sample` (`capture_mf.rs:455-576`) の `match pixel_format` に `PixelFormat::Mjpeg` 分岐を追加。既存アーム順 `Nv12 / I420 / Yuy2 / Unknown(_)` に対し、**`Yuy2` アームの直後、`Unknown(_)` アームの直前** に挿入。cfg ガードは付けず、常にコンパイルされる:

```rust
PixelFormat::Mjpeg => {
    // 本分岐に到達するのはバグ。pixel_format_to_guid(Mjpeg) = None により
    // get_configured_format で UnsupportedPixelFormat(Mjpeg) が先に返るため。
    // 万一到達しても安静にドロップせず、明示的に異常系として扱う。
    unreachable!("MJPEG reached process_sample despite pixel_format_to_guid returning None");
}
```

`process_sample` の match 分岐に到達する前に、`_buf_guard = BufGuard { buffer }` (486 行) で `buffer` は既に `BufGuard` に所有権が移動している。`unreachachable!()` マクロは最適化ビルドで undefined behavior に展開される可能性があるため、`unsafe { std::hint::unreachable_unchecked() }` は使用しない。`BufGuard` の `Drop` が自動で `Unlock` するため、リソース解放は保証される。

### 9. `examples/camera_preview.rs`

`enqueue_video_frame` の `match frame.pixel_format` に `PixelFormat::Mjpeg` 分岐を追加。cfg ガードは付けない:

```rust
PixelFormat::Mjpeg => {
    static WARN: Once = Once::new();
    WARN.call_once(|| {
        eprintln!("camera_preview: MJPEG format is not supported in this sample");
    });
    return Ok(());
}
```

`Err(...)` ではなく `eprintln!` + `Ok(())` とする。`raw_player::Error` が `From<String>` を実装しているとは限らないため、文字列エラーを返さない方針とする。

### 10. `README.md`

`## Linux の feature` セクションの末尾 (34 行目付近、「これらは両方のフラグを指定することも可能です。」の段落の直後) に以下を追記:

```markdown
### `mjpeg` feature

Linux (V4L2) で MJPEG フォーマットのパススルーキャプチャを利用可能にする feature です。
`pixel_format = Some(PixelFormat::Mjpeg)` を指定することで MJPEG カメラから JPEG ペイロードを直接受け取れます。V4L2 バックエンドが必要なため、`mjpeg` feature は自動的に `v4l2` を有効化します。

MJPEG 対応外の環境 (macOS / Windows / PipeWire) で `PixelFormat::Mjpeg` を指定すると `Error::UnsupportedPixelFormat(PixelFormat::Mjpeg)` を返します。

```bash
# MJPEG 対応を有効化してビルド
cargo build -p shiguredo_video_device --features mjpeg
```
```

### 11. `CHANGES.md`

`## develop` セクション内、全 `[ADD]` エントリの末尾の直後・全 `[CHANGE]` エントリの先頭の直前に追記する（規約の種別順 UPDATE → ADD → CHANGE → FIX に従う）。`### misc` サブセクションより前に配置する。具体的には、現在の「`[CHANGE]` VideoDevice, VideoDeviceList, VideoCapture を構造体から enum に変更する」の 1 行前に以下のエントリを追加:（CHANGES.md の構成が変わった場合は、種別順を守るよう実装者判断で位置調整する）

```
- [CHANGE] PixelFormat に Mjpeg バリアントを追加し、Linux (V4L2) で mjpeg feature による MJPEG パススルーキャプチャに対応する
  - @voluntas
```

### 12. テスト

#### 12a. PBT (`pbt/tests/prop_types.rs`)

`pbt/tests/prop_types.rs` を新規作成する。PBT では `pub` な API のみ検証可能であるため、`to_raw` / `name` / `Display` のプロパティと `VideoFrame::to_owned` / `VideoFrameOwned::as_frame` のラウンドトリップを検証する:

- `to_raw()` の戻り値が `name()` と矛盾しない (Nv12/Yuy2/I420/Mjpeg の FourCC が期待値と一致)
- `name()` が空文字列を返さない
- `Display` 出力が `name()` を含む
- `VideoFrame::to_owned` / `VideoFrameOwned::as_frame` のラウンドトリップ: MJPEG を含む全フォーマット

`from_raw` は `pub(crate)` のため PBT からアクセス不可。`from_raw` の検証は 12b の内部単体テストのみで行う。

`prop_types.rs` で `PixelFormat` に `Arbitrary` を実装する。strategy には `prop_oneof!` で `Just(PixelFormat::Nv12)` / `Just(PixelFormat::Yuy2)` / `Just(PixelFormat::I420)` / `Just(PixelFormat::Mjpeg)` から一様選択する。`PixelFormat::Unknown(u32)` は内部利用のみのため strategy から除外する。`VideoFrame` のラウンドトリップは `VideoFrameOwned` 経由で strategy を構築し、`to_owned` → `as_frame` でラウンドトリップ検証する。MJPEG variant では `stride: 0`, `stride_uv: 0`, `uv_data: None` を strategy が強制することで、誤った値でのラウンドトリップ偽陽性を防止する。

プロジェクト初の PBT 導入となる。`Cargo.toml` への `proptest` dev-dependency 追加と `[[test]]` セクション追加が必要。`pbt/tests/` ディレクトリは新規作成する。

#### 12b. 単体テスト (`src/types.rs::#[cfg(test)] mod tests` を新規作成)

`src/types.rs` には現在 `#[cfg(test)] mod tests` が存在しないため、新規作成する。`from_raw` は `pub(crate)` かつ `#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]` ガード付きのため、結合テストからは呼べず内部テストが必要:

1. `PixelFormat::from_raw(0x47504A4D)` が `PixelFormat::Mjpeg` を返す。このテスト関数には `#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]` を付ける（`from_raw` と同じ cfg 条件でコンパイルする必要がある）
2. `from_raw` → `to_raw` の MJPEG ラウンドトリップ (`PixelFormat::from_raw(0x47504A4D).to_raw() == 0x47504A4D`)。同様に `#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]` ガードを付ける。PBT は `from_raw` (`pub(crate)`) にアクセスできないため、このラウンドトリップは単体テストでのみ検証可能

`to_raw` / `name` / `Display` の MJPEG ケース検証は PBT (12a) でカバーされるため、単体テストでは重複検証しない（AGENTS.md「PBT でカバーできるものを単体テストで書かない」に従う）。`VideoFrame::to_owned` / `VideoFrameOwned::as_frame` の MJPEG ラウンドトリップも PBT (12a) でカバーされるため、`tests/test_types.rs` に MJPEG 固有の追加テストは不要とする。

#### 12c. 単体テスト (`src/frame_math.rs::#[cfg(test)] mod tests` に追加)

`src/frame_math.rs::#[cfg(test)] mod tests` (74-162 行) へのヘルパ単体テスト追加 (計 2 件、いずれも `#[cfg(enable_mjpeg)]` ガード)。既存命名規約に揃える:

1. `mjpeg_rejects_non_positive_payload_size`: `mjpeg_payload_bytes(0)` / 負値が `None`
2. `mjpeg_payload_bytes_returns_input`: `mjpeg_payload_bytes(1024)` が `Some(1024)`

#### 12d. Fuzzing 計画

本 issue の新規コードパスのうち、以下の純粋関数は実機依存せずに fuzzing 可能なため、最低限の fuzzing を実施する:

- `frame_math::mjpeg_payload_bytes` への任意 `i32` 入力（負値、0、正値、`i32::MAX`、`i32::MIN` でパニックしないことの検証）

`fuzz/fuzz_targets/` ディレクトリ（既存なければ `cargo fuzz init` で新規作成）に `mjpeg_payload_bytes.rs` を追加する。プロジェクトに `cargo-fuzz` インフラが未導入の場合は、以下を実施する（CLAUDE.md の「Fuzzing は cargo-fuzz を使うこと」に従う）:

```bash
cargo fuzz init              # 初回のみ。fuzz/ ディレクトリと fuzz/Cargo.toml が生成される
cargo fuzz add mjpeg_payload_bytes  # fuzz/fuzz_targets/mjpeg_payload_bytes.rs を生成
```

`frame_callback` の `slice::from_raw_parts(data, data_size)` 呼び出しは C 側の生ポインタを扱うため、安全な fuzz harness の設計が困難。本 issue では `mjpeg_payload_bytes` の純粋関数 fuzzing のみとし、unsafe 境界の fuzz は別途実施する。実機 MJPEG カメラの fuzzing も本 issue のスコープ外とする。

#### 12e. `tests/test_capture.rs` 修正

（旧 12e から番号変更）`tests/test_capture.rs:70` の `assert!(frame.stride > 0)` は、既定フォールバック (NV12 → YUYV) では MJPEG が選択されないため、`mjpeg` feature 有効化時も影響を受けない。修正不要。

### 13. `src/video_avf.m` / `src/video_pipewire.c`

修正不要。本 issue のスコープ外。非 V4L2 バックエンドでの MJPEG 挙動はスコープセクションに明記済み。

## 完了条件 (ファイル単位)

- [ ] `Cargo.toml`: `mjpeg = ["v4l2"]` feature 追加、`[dev-dependencies]` に `proptest` 追加、`[[test]]` セクションで `pbt/tests/prop_types.rs` を登録。`default` や `default-*` 変更なし
- [ ] `build.rs`: `check-cfg` 群に `enable_mjpeg` を追記。Linux `"linux"` アーム内の V4L2 cfg 発行直後で `CARGO_FEATURE_MJPEG` 検出時に `println!("cargo::rustc-cfg=enable_mjpeg")` を発行。`build_linux_v4l2` で `CARGO_FEATURE_MJPEG` 検出時に `cc::Build::define("SHIGUREDO_VIDEO_DEVICE_MJPEG", "1")` を呼ぶ
- [ ] `src/video.h`: `VIDEO_PIXEL_FORMAT_MJPG` 定数 (feature ガードなし)、`FrameCallback` の MJPEG 仕様 (stride 単位が pixel_format に依存する旨を含む明示的契約) コメントを追加
- [ ] `src/video_v4l2.c`: ファイル先頭に `#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG` ガード付きで `MJPEG_MAX_PAYLOAD_BYTES` (256 MiB、根拠コメント付き) を定義。`convert_v4l2_pixel_format` / `convert_video_pixel_format_to_v4l2` / `capture_thread` の MJPEG 関連コードを `#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG` で囲む。`capture_thread` の MJPEG 分岐は YUYV の `else if` の後ろに配置し、絶対上限ガードを含む
- [ ] `src/types.rs`: `VIDEO_PIXEL_FORMAT_MJPG` 定数 (cfg ガードなし・OS 条件なし)、`PixelFormat::Mjpeg` バリアント (cfg ガードなし)、`from_raw` / `to_raw` / `name` / `Display` / `pixel_format_to_guid` の match 分岐 (すべて cfg ガードなし) を追加。`PixelFormat` enum の doc comment (`to_raw` は全プラットフォーム利用可能、`from_raw` は FFI バックエンド有効時のみ) を修正。`VideoFrame` / `VideoFrameOwned` の `stride` / `stride_uv` フィールド doc と `VideoCaptureConfig` の rustdoc に MJPEG 時の解釈、`V4L2_BUF_FLAG_ERROR` 警告、非対応環境でのエラー型統一を明記
- [ ] `src/lib.rs`: クレートドキュメントに `mjpeg` feature 有効化時の MJPEG 既知扱いと `closed/0017` 方針維持を独立 2 文で追記
- [ ] `src/frame_math.rs`: `#[cfg(enable_mjpeg)]` ガード付きで `mjpeg_payload_bytes` ヘルパ (引数名 `payload_size: i32`、用途コメント付き) を追加。`#[cfg(test)] mod tests` に単体テスト 2 件を追加
- [ ] `src/capture_ffi.rs`: `FfiCaptureImpl::new` 内に `#[cfg(not(enable_mjpeg))]` ガード付きで MJPEG 要求時実行時エラーを追加。`FfiCaptureImpl::new_pipewire` の先頭に MJPEG 拒否を追加。`frame_callback` の `match pf` に cfg ガードなしの `PixelFormat::Mjpeg` 分岐を追加 (内部を cfg で分岐)
- [ ] `src/capture_mf.rs`: `process_sample` の `match` に cfg ガードなしの `PixelFormat::Mjpeg => return,` を追加 (BufGuard が Drop で Unlock するため明示的 Unlock 不要)
- [ ] `examples/camera_preview.rs`: `enqueue_video_frame` の `match` に `PixelFormat::Mjpeg` 分岐を追加 (ログは英語、`Once` で 1 回のみ出力、`return Ok(())`)
- [ ] `tests/test_types.rs`: 修正不要。`to_owned` / `as_frame` の MJPEG ラウンドトリップは PBT (12a) で検証されるため追加テスト不要
- [ ] `src/types.rs` 内に `#[cfg(test)] mod tests` を新規作成し、`from_raw` のテスト 2 件を追加
- [ ] `pbt/tests/prop_types.rs` (新規作成。`pbt/tests/` ディレクトリも新規作成): `PixelFormat` ラウンドトリップ PBT に `Mjpeg` バリアントを追加。`VideoFrame::to_owned` / `VideoFrameOwned::as_frame` のラウンドトリップ PBT を追加
- [ ] `fuzz/fuzz_targets/mjpeg_payload_bytes.rs` (新規作成。`cargo fuzz init` による `fuzz/` ディレクトリ生成に続き `cargo fuzz add mjpeg_payload_bytes` で生成): `mjpeg_payload_bytes` への任意 `i32` 入力に対するパニック安全性を検証
- [ ] `README.md`: `## Linux の feature` セクションに `mjpeg` feature の説明を追記
- [ ] `CHANGES.md`: `## develop` セクションの全 `[ADD]` エントリ末尾直後・全 `[CHANGE]` エントリ先頭直前に新規 `[CHANGE]` で MJPEG feature 追加エントリを挿入（種別順 UPDATE → ADD → CHANGE → FIX に従う）
- [ ] ローカル (Linux 環境) で以下の組み合わせで `cargo build` / `cargo clippy --all-targets` / `cargo test` がすべて通る。`cargo clippy` は **MJPEG 対応に起因する新規 warning が 0 件であること** を確認条件とする（既存 warning は対象外）。対象は `src/types.rs` 内 `mod tests` の MJPEG 関連 2 件、`src/frame_math.rs` 内 `mod tests` の MJPEG 関連 2 件:
  - `cargo build` / `cargo test` (default features)
  - `cargo build --no-default-features --features v4l2` / `cargo test --no-default-features --features v4l2`
  - `cargo build --features mjpeg` / `cargo test --features mjpeg`
  - `cargo build --no-default-features --features v4l2,mjpeg` / `cargo test --no-default-features --features v4l2,mjpeg`
- [ ] Linux / macOS / Windows の全プラットフォームビルドを CI で確認する (ローカルでの Windows / macOS 検証は不要)

## 影響範囲

- **`mjpeg` feature 無効時 (既定)**: `build.rs` が `SHIGUREDO_VIDEO_DEVICE_MJPEG` を define しないため C 側で MJPEG case が含まれず、Rust 側のキャプチャ初期化では実行時エラーを返す。`PixelFormat::Mjpeg` は常に定義されるため match 網羅性は全プラットフォームで保たれる。`from_raw(0x47504A4D)` は `PixelFormat::Mjpeg` を返すが、利用者が明示的に `Some(PixelFormat::Mjpeg)` を指定しない限り影響はない。MJPEG カメラの `formats()` 結果は引き続き空 (C 側でフィルタされるため)
- **`mjpeg` feature 有効時 (Linux)**: MJPEG カメラを `VideoDevice::formats()` で列挙でき、`Some(PixelFormat::Mjpeg)` でキャプチャ可能。`VideoFrame::data` に JPEG ペイロード、`stride / stride_uv` は 0、`uv_data` は None
- **`mjpeg` feature 有効時 (macOS)**: `Some(PixelFormat::Mjpeg)` 指定時は `Error::UnsupportedPixelFormat(Mjpeg)` を返す (`FfiCaptureImpl::new` の `#[cfg(not(enable_mjpeg))]` ガード)。`formats()` には MJPEG は出現しない (`video_avf.m::convert_pixel_format` が MJPEG を `default: 0` で除外)
- **`mjpeg` feature 有効時 (Windows)**: `mjpeg = ["v4l2"]` により `CARGO_FEATURE_MJPEG` は cargo によりセットされるが、build.rs の cfg 発行 (`enable_mjpeg`) は Linux アーム内のみのため Windows では発行されない。`Some(PixelFormat::Mjpeg)` 指定時は `capture_ffi.rs` がコンパイルされない (Windows では `enable_v4l2`/`enable_avf`/`enable_pipewire` のいずれも発行されないため) ため、`FfiCaptureImpl` のチェックは存在せず、MF 側の `pixel_format_to_guid(Mjpeg) = None` → `UnsupportedPixelFormat(Mjpeg)` の経路で拒否される。`formats()` には MJPEG は出現しない (`guid_to_pixel_format` を変更しないため)
- **C ABI**: シグネチャは変更なし。`FrameCallback` の `stride` 引数の意味が `pixel_format` に依存する契約を明示文書化する
