# VideoDevice, VideoDeviceList, VideoCapture を trait 化する

Created: 2026-05-26
Model: DeepSeek v4 Pro

## 問題

### 1. v4l2 と pipewire が共存できない

現在 `v4l2` と `pipewire` の feature flag は相互排他であり、`build.rs` で同時に有効化すると `panic!` する。
同一バイナリで両バックエンドを切り替えたり、実行時に選択することができない。

### 2. 同一インターフェースの保証がない

各プラットフォーム/バックエンド（macOS, Windows, V4L2, PipeWire）は同じシグネチャのメソッドを持つが、
`#[cfg]` による条件コンパイルで切り替えているだけで、trait による型レベルの保証がない。
片方のバックエンドにメソッドを追加してもう片方に追加し忘れてもコンパイルが通ってしまう。

### 3. VideoDevice と VideoDeviceList のライフタイム制約が表現されていない

V4L2 や PipeWire など FFI 経由のバックエンドでは、`VideoDevice` は `VideoDeviceList` が所有する
C 側のメモリを参照しているため、`VideoDevice` は `VideoDeviceList` より短いライフタイムでなければならない。
現在の `VideoDevice` はライフタイムパラメータを持たないため、将来的に `take_device()` のような
デバイスを所有権移動で取り出す API を追加した場合に、dangling pointer を型レベルで防げない。

## 提案

### trait の定義

```rust
pub trait VideoDevice: Send + Sync {
    fn name(&self) -> Result<String>;
    fn unique_id(&self) -> Result<String>;
    fn format_count(&self) -> usize;
    fn formats(&self) -> Vec<VideoFormat>;
}

pub trait VideoDeviceList: Send + Sync {
    type Device<'a>: VideoDevice + 'a
    where
        Self: 'a;

    fn devices(&self) -> &[Self::Device<'_>];
    fn len(&self) -> usize {
        self.devices().len()
    }
    fn is_empty(&self) -> bool {
        self.devices().is_empty()
    }
}

pub trait VideoCapture {
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self);
    fn config(&self) -> &VideoCaptureConfig;
}
```

#### trait 設計の根拠

- `VideoDeviceList` は GAT を使い、`devices(&self)` の戻り値を `&self` の借用に束縛することで、リストが drop された後にデバイスを使うことをコンパイル時に防ぐ
- FFI ベースのバックエンドでは `Device<'a>` を `V4l2VideoDevice<'a>` 等にする。`PhantomData<&'a ()>` は共変（`'static: 'a` なら `T<'static>` を `T<'a>` として扱える）であるため、内部で `Vec<V4l2VideoDevice<'static>>` として保持し `&[V4l2VideoDevice<'_>]` として返すライフタイム縮退が安全に成立する
- Windows のようにデータを完全所有するバックエンドでは `Device<'a>` = `MfVideoDevice`（`'a` を無視）
- `VideoDeviceList` に `enumerate()` を含まない理由: 列挙はバックエンド固有の初期化（COM 初期化等）を伴うため、trait で抽象化するメリットが薄い。呼び出し側は `fn do_something(list: &impl VideoDeviceList)` のように列挙済みのリストを受け取る形で利用する
- GAT を使う trait は object-safe ではないため `Box<dyn VideoDeviceList>` や `&dyn VideoDeviceList` による動的ディスパッチは不可。バックエンド選択は `#[cfg]` による静的分岐で行う
- `where Self: 'a` 制約の意味: `devices(&self)` が返す `&[Self::Device<'a>]` のライフタイム `'a` を `&self` の借用期間に束縛する。これにより `VideoDeviceList` が drop された後に `Device<'a>` を使用するコードはコンパイルエラーになる。FFI バックエンドでは C 側メモリを参照する `Device<'a>` がリストより長生きすると dangling pointer になるため、型レベルでこれを防ぐ
- `VideoDevice` と `VideoDeviceList` に `Sync` 境界を付ける理由: FFI 側のデバイスデータは read-only であり複数スレッドからの同時読み出しが安全。`Arc<impl VideoDeviceList>` でスレッド間共有する利用パターンを許容するため。`VideoCapture` は `&mut self` メソッドを持つため `Sync` は不要
- `name()` / `unique_id()` が `Result<String>` を返す理由: FFI バックエンドでは C 側がヌルポインタを返しうるため。Windows バックエンドでは常に `Ok(...)` を返すが、trait の統一性のため `Result` で揃える
- `stop()` が `Result` を返さない理由: デバイス切断等で停止処理自体が失敗しても、呼び出し側にリカバリ手段がない。リソース解放は `Drop` で保証されるため、失敗は内部で吸収する
- `format_count()` は C 側が報告するエントリ数であり、`formats()` が返すベクタの長さはそれ以下になりうる（C 側がスキップする場合）。この差異は trait のドキュメントで明示する
- `formats()` が `Vec<VideoFormat>` を返す理由: FFI バックエンドでは C 側の `VideoFormatEntry` から変換コピーが必要であり、参照を返せない。Windows バックエンドでは内部 Vec の clone になるが、フォーマット列挙はデバイス初期化時に 1 回呼ぶだけの低頻度操作であり、性能上問題にならない

#### `VideoCapture::start` のセマンティクス

- `start` は冪等とする。既に running 状態であれば `Ok(())` を返す
- `stop` 後の再 `start` は許容する（再開可能な設計）
- `stop` が running でない状態の場合は no-op
- `stop` はブロッキングする。全バックエンドでキャプチャスレッド/コールバックの完了を待機してから復帰する（V4L2: `pthread_join`、PipeWire: `pw_thread_loop_stop`、macOS: `dispatch_sync` + `stopRunning`、Windows: `thread::join`）
- `Drop` は `stop()` を呼んでからリソースを解放する（各具象型の責務）
- 注意: PipeWire バックエンドの `start()` は内部でストリーミング状態になるまでブロックする（`pw_thread_loop_wait` ループ）。他バックエンドの `start()` は即座に復帰する。この差異は trait の doc comment に明記する
- Windows バックエンドの再 start 対応（既存バグ修正を含む）: 現行の `capture_windows.rs` では `start()` 内で `self.callback.take()` により callback の所有権をスレッドに move しているため、`stop()` 後に再度 `start()` しても callback が `None` になり何も開始されない。これは trait の「stop 後の再 start を許容する」セマンティクスに違反する既存バグである。修正方針として、キャプチャスレッドの `JoinHandle<VideoFrameCallback>` 戻り値でコールバックを回収し、`stop()` 時に `self.callback` に復元する。具体的な `MfVideoCapture` の新構造体設計は後述

### バックエンド別の具象型

| バックエンド | デバイスリスト型 | デバイス型 | キャプチャ型 | feature flag |
|---|---|---|---|---|
| V4L2 | `V4l2VideoDeviceList` | `V4l2VideoDevice<'a>` | `V4l2VideoCapture` | `v4l2` |
| PipeWire | `PipewireVideoDeviceList` | `PipewireVideoDevice<'a>` | `PipewireVideoCapture` | `pipewire` |
| macOS (AVFoundation) | `AvfVideoDeviceList` | `AvfVideoDevice<'a>` | `AvfVideoCapture` | (常時、macOS のみ) |
| Windows (Media Foundation) | `MfVideoDeviceList` | `MfVideoDevice` | `MfVideoCapture` | (常時、Windows のみ) |

### FFI バックエンドの実装方針

シンボル名がバックエンド別に異なるため、各バックエンドモジュールに直接実装を書く。バックエンド間でコード構造が類似するがマクロは使用しない。

各バックエンドモジュールの責務:
- `device_v4l2.rs`: `V4l2VideoDevice<'a>`, `V4l2VideoDeviceList` の `VideoDevice` / `VideoDeviceList` trait 実装。`ffi_v4l2` モジュールの関数を直接呼び出す
- `device_avf.rs`: `AvfVideoDevice<'a>`, `AvfVideoDeviceList` の trait 実装。`ffi_avf` の関数を呼び出す
- `device_pipewire.rs`: `PipewireVideoDevice<'a>`, `PipewireVideoDeviceList` の trait 実装。`ffi_pipewire` の関数を呼び出す
- `capture_v4l2.rs`: `V4l2VideoCapture` の trait 実装 + `extern "C" fn frame_callback`。`ffi_v4l2` の関数を呼び出す
- `capture_avf.rs`: `AvfVideoCapture` の trait 実装 + `extern "C" fn frame_callback`。`ffi_avf` の関数を呼び出す
- `capture_pipewire.rs`: `PipewireVideoCapture` の trait 実装 + `extern "C" fn frame_callback`。`ffi_pipewire` の関数を呼び出す

`CaptureContext` 構造体は 3 ファイルに重複定義する（マクロ不使用のため。コード構造は同一だが、FFI 型への依存がバックエンド別に異なる）:

コールバック関数は各モジュールに同名（`frame_callback`）で定義する。`extern "C"` は呼出規約の指定であり、`#[no_mangle]` を付けない限りシンボル名は Rust のマングリング（モジュールパスを含む）が適用される。そのため `capture_v4l2::frame_callback` と `capture_avf::frame_callback` は異なるリンカシンボルとなり衝突しない。コールバック内の `PixelFormat` 分岐ロジックは 3 バックエンドで実質同一だが、マクロを使わず各ファイルに直接書く。フレームサイズ計算は `frame_math` モジュールの関数を呼び出すことで重複を最小化する。

共有ユーティリティ:
- `src/frame_math.rs`: フレームサイズ計算（コールバックから呼ばれる）
- `src/types.rs`: `PixelFormat`, `VideoCaptureConfig` 等の共通型
- `src/error.rs`: エラー型

注意: PipeWire のフォーマット列挙は現在未実装（`video_pipewire.c` で `format_count = 0`）であるため、`PipewireVideoDevice` の `format_count()` は 0、`formats()` は空 Vec を返す。この既知の制限は trait のドキュメントに明記する。

### FFI シンボル衝突の解決

現在 `video_v4l2.c` と `video_pipewire.c` は同名の関数をエクスポートしており、同時にリンクするとシンボルが衝突する。

#### 解決策: C 関数名にバックエンド接頭辞を付加する

リネーム対象の全関数:

| 現在の関数名 | V4L2 | PipeWire | macOS |
|---|---|---|---|
| `video_enumerate_devices` | `video_v4l2_enumerate_devices` | `video_pipewire_enumerate_devices` | `video_avf_enumerate_devices` |
| `video_free_devices` | `video_v4l2_free_devices` | `video_pipewire_free_devices` | `video_avf_free_devices` |
| `video_device_name` | `video_v4l2_device_name` | `video_pipewire_device_name` | `video_avf_device_name` |
| `video_device_unique_id` | `video_v4l2_device_unique_id` | `video_pipewire_device_unique_id` | `video_avf_device_unique_id` |
| `video_device_format_count` | `video_v4l2_device_format_count` | `video_pipewire_device_format_count` | `video_avf_device_format_count` |
| `video_device_get_format` | `video_v4l2_device_get_format` | `video_pipewire_device_get_format` | `video_avf_device_get_format` |
| `video_session_create` | `video_v4l2_session_create` | `video_pipewire_session_create` | `video_avf_session_create` |
| `video_session_destroy` | `video_v4l2_session_destroy` | `video_pipewire_session_destroy` | `video_avf_session_destroy` |
| `video_session_start` | `video_v4l2_session_start` | `video_pipewire_session_start` | `video_avf_session_start` |
| `video_session_stop` | `video_v4l2_session_stop` | `video_pipewire_session_stop` | `video_avf_session_stop` |

ヘッダ分割:
- `video_c.h` を廃止し `video_v4l2.h`, `video_pipewire.h`, `video_avf.h` に分割する
- `video_common.h` には以下を残し、各バックエンドヘッダから `#include` する:
  - `VideoDevice`、`VideoSession` の前方宣言（`struct VideoDevice;` / `struct VideoSession;`。実体定義は各 `.c`/`.m` ファイル内）
  - `VideoFormatEntry` 構造体定義
  - `FrameCallback` 型定義
  - `VIDEO_PIXEL_FORMAT_*` 定数
- 各バックエンドヘッダ（`video_v4l2.h` 等）は `video_common.h` を include し、自バックエンドの `video_<backend>_*` 関数プロトタイプのみを宣言する。他バックエンドの関数は一切宣言しない（bindgen で不要なシンボルが生成されるのを防ぐため）

### 実装例

```rust
// FFI バックエンド (V4L2)
pub struct V4l2VideoDevice<'a> {
    ptr: NonNull<ffi_v4l2::VideoDevice>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> VideoDevice for V4l2VideoDevice<'a> {
    fn name(&self) -> Result<String> { /* FFI 呼び出し */ }
    fn unique_id(&self) -> Result<String> { /* FFI 呼び出し */ }
    fn format_count(&self) -> usize { /* FFI 呼び出し */ }
    fn formats(&self) -> Vec<VideoFormat> { /* FFI 呼び出し */ }
}

// SAFETY: FFI 側の VideoDevice はデバイスリストが所有する read-only データであり、
// 複数スレッドから同時に読むことは安全。
unsafe impl<'a> Send for V4l2VideoDevice<'a> {}
unsafe impl<'a> Sync for V4l2VideoDevice<'a> {}

pub struct V4l2VideoDeviceList {
    devices: Vec<V4l2VideoDevice<'static>>,
    raw_devices: *mut *mut ffi_v4l2::VideoDevice,
    // C が返した全エントリ数。devices.len() とは異なる場合がある（NULL エントリ除外のため）。
    // Drop で video_v4l2_free_devices に渡す際はこの値を使う（C 側の確保サイズに対応）。
    count: i32,
}

impl VideoDeviceList for V4l2VideoDeviceList {
    type Device<'a> = V4l2VideoDevice<'a>;

    fn devices(&self) -> &[V4l2VideoDevice<'_>] {
        &self.devices
    }
}

impl V4l2VideoDeviceList {
    pub fn enumerate() -> Result<Self> { /* FFI 呼び出し */ }
}

impl Drop for V4l2VideoDeviceList {
    fn drop(&mut self) {
        unsafe { ffi_v4l2::video_v4l2_free_devices(self.raw_devices, self.count); }
    }
}

// SAFETY: FFI 側のデバイスリストは read-only であり、スレッド間共有は安全。
unsafe impl Send for V4l2VideoDeviceList {}
unsafe impl Sync for V4l2VideoDeviceList {}

// Windows バックエンド (データを完全所有)
pub struct MfVideoDevice {
    name: String,
    unique_id: String,
    formats: Vec<VideoFormat>,
}

impl VideoDevice for MfVideoDevice {
    fn name(&self) -> Result<String> { Ok(self.name.clone()) }
    fn unique_id(&self) -> Result<String> { Ok(self.unique_id.clone()) }
    fn format_count(&self) -> usize { self.formats.len() }
    fn formats(&self) -> Vec<VideoFormat> { self.formats.clone() }
}

pub struct MfVideoDeviceList {
    devices: Vec<MfVideoDevice>,
}

impl VideoDeviceList for MfVideoDeviceList {
    type Device<'a> = MfVideoDevice;

    fn devices(&self) -> &[MfVideoDevice] {
        &self.devices
    }
}

impl MfVideoDeviceList {
    pub fn enumerate() -> Result<Self> { /* Media Foundation 列挙 */ }
}
```

### キャプチャ生成

全バックエンドで同一のシグネチャ:

```rust
impl V4l2VideoCapture {
    pub fn new<F>(config: VideoCaptureConfig, callback: F) -> Result<Self>
    where F: Fn(VideoFrame<'_>) + Send + 'static;
}
// AvfVideoCapture, PipewireVideoCapture, MfVideoCapture も同一シグネチャ
```

#### `new()` を trait に含めない理由

`new()` は全バックエンドで同一シグネチャだが、trait に含めない。理由:
- コンストラクタはバックエンド固有の初期化処理（COM 初期化、FFI セッション確保、macOS の権限ダイアログブロッキング等）を伴い、trait で抽象化するメリットが薄い
- `VideoDeviceList::enumerate()` と同じ理由（trait のセマンティクスとしては「既に構築済みのキャプチャオブジェクトに対する操作」を定義する）
- GAT を使う `VideoDeviceList` と同様に object-safe ではない trait にコンストラクタを入れても `Box<dyn VideoCapture>` で使えないため利用価値が低い

#### macOS バックエンドの running フラグ設計

macOS の FFI 側（`video_c.m` の `video_session_start`）は running 状態を一切チェックせずに毎回 `startRunning` を呼ぶ。trait の `start` 冪等性を Rust 側で保証するため、`AvfVideoCapture` は `CaptureContext` 内の `running: AtomicBool` を使う（現行 `capture.rs` と同様）。構造体自体に別途 `running` フィールドは持たない:

```rust
struct CaptureContext {
    callback: Box<dyn Fn(VideoFrame<'_>) + Send + 'static>,
    running: AtomicBool,
}

pub struct AvfVideoCapture {
    session: NonNull<ffi_avf::VideoSession>,
    context: Box<CaptureContext>,
    config: VideoCaptureConfig,
}
```

`start()` は `self.context.running.load(Acquire)` が `true` なら即座に `Ok(())` を返す。`stop()` は `self.context.running` が `false` なら no-op。

`V4l2VideoCapture` と `PipewireVideoCapture` も同構造（`CaptureContext` を用い、`running` は `CaptureContext` 内の `AtomicBool` を使用する）:

```rust
pub struct V4l2VideoCapture {
    session: Option<NonNull<ffi_v4l2::VideoSession>>,
    context: Option<Box<CaptureContext>>,
    config: VideoCaptureConfig,
}

pub struct PipewireVideoCapture {
    session: Option<NonNull<ffi_pipewire::VideoSession>>,
    context: Option<Box<CaptureContext>>,
    config: VideoCaptureConfig,
}
```

### `MfVideoCapture` の新構造体設計

現行の callback を `take()` で消費して戻せなくなる問題を、スレッド終了時にコールバックを返却する方式で解決する:

```rust
type VideoFrameCallback = Box<dyn Fn(VideoFrame<'_>) + Send + 'static>;

pub struct MfVideoCapture {
    callback: Option<VideoFrameCallback>,
    session: Option<SessionData>,
    running: Arc<AtomicBool>,
    capture_thread: Option<thread::JoinHandle<VideoFrameCallback>>,
    config: VideoCaptureConfig,
    _com_guard: CoInitGuard,
}
```

- `start()`: `self.callback.take()` でコールバックをスレッドに移動する。`source_reader` は `SendPtr` でラップして渡す（現行と同じ）
- スレッド関数の戻り値型を `VideoFrameCallback` にする。ループ終了後にコールバックを return する
- `stop()`: `running` を `false` にした後 `thread::join()` でスレッドを回収し、戻り値のコールバックを `self.callback` に復元する
- 再 `start()` 時は復元済みの `self.callback` から再度 `take()` して新スレッドに渡す
- キャプチャスレッド内では `CoInitGuard::new()` を呼び MTA を初期化する（スレッド毎の COM 初期化は MF の要件）
- `SendPtr` ラッパーは `source_reader` のスレッド間移動に使用する（現行と同じ用途）
- コールバック型の `Send` 境界は変更なし（`Sync` は不要）

#### `Drop` 実装

```rust
impl Drop for MfVideoCapture {
    fn drop(&mut self) {
        self.stop();
        if let Some(session) = self.session.take() {
            unsafe { let _ = media_source.Shutdown(); }
        }
        unsafe { let _ = MFShutdown(); }
    }
}
```

`_com_guard: CoInitGuard` の `Drop` が `CoUninitialize` を呼ぶ。COM オブジェクトの解放は `CoInitializeEx` ～ `CoUninitialize` の範囲内で安全に行われる（`!Send` によりスレッド移動なし）。

#### `MfShutdownGuard` の削除

現行 `capture_windows.rs` の `MfShutdownGuard` は `new()` のエラーパスで `MFShutdown` を漏らさないための仕組みだが、`_com_guard` を構造体フィールドに保持する新設計では `new()` のエラーパスで `Drop` が呼ばれない（エラー時は `Self` が構築されていない）。代わりに `new()` 内で `MFShutdown` を inline で呼ぶ形に変更する:

### feature flag の変更

- `v4l2` と `pipewire` の相互排他制約を撤廃し、両方を同時に有効化可能にする
- `default = ["v4l2"]` は維持
- 両方有効な場合、ユーザは `V4l2VideoDeviceList::enumerate()` と `PipewireVideoDeviceList::enumerate()` のいずれかを明示的に呼ぶ

### build.rs の変更

現行の `generate_bindings` は単一ヘッダ・単一出力だが、バックエンド別に呼び分けるようシグネチャを変更する:

```rust
fn generate_bindings(header: &Path, out_file: &str, out_dir: &Path) {
    let bindings = bindgen::Builder::default()
        .header(header.to_str().unwrap())
        .allowlist_function("video_.*")
        .allowlist_type("VideoDevice")
        .allowlist_type("VideoSession")
        .allowlist_type("VideoFormatEntry")
        .allowlist_type("FrameCallback")
        .allowlist_var("VIDEO_PIXEL_FORMAT_.*")
        .derive_default(true)
        .derive_debug(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Failed to generate bindings");
    bindings.write_to_file(out_dir.join(out_file)).expect("Failed to write bindings");
}
```

各 `ffi_*.rs` は対応する bindings ファイルを include する:
- `src/ffi_v4l2.rs`: `include!(concat!(env!("OUT_DIR"), "/bindings_v4l2.rs"));`
- `src/ffi_pipewire.rs`: `include!(concat!(env!("OUT_DIR"), "/bindings_pipewire.rs"));`
- `src/ffi_avf.rs`: `include!(concat!(env!("OUT_DIR"), "/bindings_avf.rs"));`

#### Linux

- 相互排他 `panic!` を削除する
- `has_v4l2` の場合 `build_linux_v4l2` を呼ぶ（`compile()` 出力名を `"video_v4l2"` に変更）
- `has_pipewire` の場合 `build_linux_pipewire` を呼ぶ（出力名 `"video_pipewire"` は変更なし）
- 両方有効な場合は両方呼ぶ
- V4L2 有効時: `generate_bindings(&src_dir.join("video_v4l2.h"), "bindings_v4l2.rs", &out_dir)`
- PipeWire 有効時: `generate_bindings(&src_dir.join("video_pipewire.h"), "bindings_pipewire.rs", &out_dir)`

#### macOS

- `build_macos` の `compile()` 出力名を `"video_avf"` に変更（現在の `"video_c"` から）
- `generate_bindings(&src_dir.join("video_avf.h"), "bindings_avf.rs", &out_dir)`

#### `rerun-if-changed` の全体設計

現在の `main()` 直下にある `println!("cargo::rerun-if-changed=src/video_c.h")` を削除し、各ビルド関数内で必要なファイルを個別に指定する:

- **Linux V4L2**: `src/video_v4l2.c`, `src/video_v4l2.h`, `src/video_common.h`
- **Linux PipeWire**: `src/video_pipewire.c`, `src/video_pipewire.h`, `src/video_common.h`
- **macOS**: `src/video_avf.m`, `src/video_avf.h`, `src/video_common.h`
- **Windows**: なし（C コンパイルも bindgen も不要）

### フレーム計算の共通関数

新設する `src/frame_math.rs`（内部モジュール）に以下の 2 系統を共存させる:

#### ストライドベース（FFI バックエンド向け）

C コールバックから stride を受け取る場合に使用:
- `nv12_plane_sizes(stride: i32, stride_uv: i32, height: i32) -> Option<(usize, usize)>`
- `i420_plane_sizes(stride: i32, stride_uv: i32, height: i32) -> Option<(usize, usize)>`
- `yuy2_packed_frame_bytes(stride: i32, height: i32) -> Option<usize>`

#### パックドバッファベース（Windows 向け）

Media Foundation の連続バッファから幅と高さで計算する場合に使用:
- `nv12_packed_frame_bytes(width: i32, height: i32) -> Option<usize>`
- `i420_packed_frame_bytes(width: i32, height: i32) -> Option<usize>`
- `yuy2_packed_frame_bytes_win(width: i32, height: i32) -> Option<usize>`

### Windows `MfVideoCapture` の `Send` 非対応

`VideoCapture` trait は `Send` 境界を持たない。FFI バックエンド（`V4l2VideoCapture` 等）は個別に `unsafe impl Send` を実装するが、`MfVideoCapture` は自動導出で `!Send` になる（`CoInitGuard` が `!Send` のため）。

#### `CoInitGuard` の `!Send` 化

`CoInitGuard` に `PhantomData<*const ()>` を追加し `!Send` にする:

```rust
#[cfg(target_os = "windows")]
pub(crate) struct CoInitGuard {
    _not_send: std::marker::PhantomData<*const ()>,
}
```

根拠: `CoInitializeEx` / `CoUninitialize` は同一スレッドで対にする必要がある。`CoInitGuard` が別スレッドに move されると、`Drop` で `CoUninitialize` が別スレッドで呼ばれ、COM のスレッド契約に違反する。`!Send` にすることでこれをコンパイル時に防ぐ。

これにより `MfVideoCapture` は `_com_guard: CoInitGuard` を持つことで自動的に `!Send` が導出され、`unsafe impl` や安全ドキュメントは不要になる。

#### COM ライフタイムの設計

- `_com_guard: CoInitGuard` を構造体に保持し、COM オブジェクト（`IMFSourceReader`、`IMFMediaSource`）の生存期間全体を `CoInitializeEx` ～ `CoUninitialize` の範囲内に収める
- `MfVideoCapture` は構築スレッドと同じスレッドで drop する（`!Send` により保証される）
- キャプチャスレッドは独自の `CoInitGuard::new()` で COM を初期化する（スレッド毎の COM 初期化は MF の要件）

### `PixelFormat` 定数の扱い

現在 `types.rs` の `VIDEO_PIXEL_FORMAT_*` 定数と `from_raw` / `to_raw` メソッドは `#[cfg(any(target_os = "macos", target_os = "linux"))]` で定義されている。FFI 分割後も定数値は全バックエンドで同一（FourCC 値）であるため:
- `VIDEO_PIXEL_FORMAT_*` 定数の `#[cfg]` を削除し、全プラットフォームで `pub(crate)` 定義する
- `from_raw` の `#[cfg]` を削除し、全プラットフォームで `pub(crate)` として利用可能にする（現在も `pub(crate)` のため変更なし）
- `to_raw` の `#[cfg]` を削除し、全プラットフォームで利用可能にする。`to_raw` は現在 `pub fn` であり、この可視性は維持する（Windows で新たにビルド対象になる = API 追加）
- `PixelFormat` の doc comment（「Windows では `to_raw` / `from_raw` はビルド対象に含まれない」）を削除する

注意: issue 0014 では trait 化前の状況で「API 追加は行わない（オプション A）」が採用された。本 issue の trait 化は全面的な破壊的変更であり、`to_raw` の全プラットフォーム公開は [ADD] として `CHANGES.md` に記載する。Windows の GUID 変換（`guid_to_pixel_format` / `pixel_format_to_guid`）は引き続き `pub(crate)` の内部関数として残す

### `lib.rs` の再エクスポート設計

```rust
// src/lib.rs

// trait は常に公開
mod types;
mod error;
mod frame_math;

pub use error::{Error, Result};
pub use types::{PixelBuffer, PixelFormat, VideoCaptureConfig, VideoFormat, VideoFrame, VideoFrameOwned};

// trait 定義は lib.rs 内に直接定義する（モジュール数が 3 つだけなので分離の必要性が薄い）
pub trait VideoDevice: Send + Sync { /* ... */ }
pub trait VideoDeviceList: Send + Sync { /* ... */ }
// VideoCapture は Send 境界を持たない（Windows バックエンドが COM スレッド束縛のため !Send）
pub trait VideoCapture { /* ... */ }

// バックエンド別の具象型を条件付きで公開
#[cfg(all(target_os = "linux", feature = "v4l2"))]
mod device_v4l2;
#[cfg(all(target_os = "linux", feature = "v4l2"))]
pub use device_v4l2::{V4l2VideoDevice, V4l2VideoDeviceList};
#[cfg(all(target_os = "linux", feature = "v4l2"))]
mod capture_v4l2;
#[cfg(all(target_os = "linux", feature = "v4l2"))]
pub use capture_v4l2::V4l2VideoCapture;

#[cfg(all(target_os = "linux", feature = "pipewire"))]
mod device_pipewire;
#[cfg(all(target_os = "linux", feature = "pipewire"))]
pub use device_pipewire::{PipewireVideoDevice, PipewireVideoDeviceList};
#[cfg(all(target_os = "linux", feature = "pipewire"))]
mod capture_pipewire;
#[cfg(all(target_os = "linux", feature = "pipewire"))]
pub use capture_pipewire::PipewireVideoCapture;

#[cfg(target_os = "macos")]
mod device_avf;
#[cfg(target_os = "macos")]
pub use device_avf::{AvfVideoDevice, AvfVideoDeviceList};
#[cfg(target_os = "macos")]
mod capture_avf;
#[cfg(target_os = "macos")]
pub use capture_avf::AvfVideoCapture;

#[cfg(target_os = "windows")]
mod device_mf;
#[cfg(target_os = "windows")]
pub use device_mf::{MfVideoDevice, MfVideoDeviceList};
#[cfg(target_os = "windows")]
mod capture_mf;
#[cfg(target_os = "windows")]
pub use capture_mf::MfVideoCapture;
```

## 破壊的変更

- `VideoDevice` 構造体 → trait に変更（`Send + Sync` 境界付き）
- `VideoCapture` 構造体 → trait に変更
- `VideoDeviceList` 構造体 → trait に変更（`Send + Sync` 境界付き）
- `VideoDeviceList::enumerate()` → `V4l2VideoDeviceList::enumerate()` 等に変更
- `VideoCapture::new()` → `V4l2VideoCapture::new()` 等に変更
- `IntoIterator for &VideoDeviceList` が削除される。`for device in &list` パターンおよび `(&list).into_iter()` チェーンは全てコンパイルエラーになる。代わりに `list.devices()` で取得したスライスをイテレートする
- trait 化により `list.len()` / `list.is_empty()` を呼ぶには trait を use する必要がある（`use shiguredo_video_device::VideoDeviceList;`）。具象型のメソッドではなく trait のデフォルト実装になるため
- `VideoCapture::config(&self) -> &VideoCaptureConfig` が trait メソッドになるため、trait を `use` していない既存コードで `capture.config()` を呼ぶとコンパイルエラーになる（`VideoCapture` 型に `.config()` が直接生えない）

### 移行例

```rust
// 変更前
let list = VideoDeviceList::enumerate()?;
for device in &list { /* ... */ }

// 変更後 (V4L2)
let list = V4l2VideoDeviceList::enumerate()?;
for device in list.devices() { /* ... */ }
```

### サンプルの `#[cfg]` 分岐パターン

サンプルは全プラットフォームで動作する「お手本」であるため、以下のように `#[cfg]` で分岐する:

```rust
#[cfg(all(target_os = "linux", feature = "v4l2"))]
use shiguredo_video_device::{V4l2VideoDeviceList, V4l2VideoCapture};
#[cfg(all(target_os = "linux", feature = "pipewire", not(feature = "v4l2")))]
use shiguredo_video_device::{PipewireVideoDeviceList, PipewireVideoCapture};
#[cfg(target_os = "macos")]
use shiguredo_video_device::{AvfVideoDeviceList, AvfVideoCapture};
#[cfg(target_os = "windows")]
use shiguredo_video_device::{MfVideoDeviceList, MfVideoCapture};

fn enumerate_devices() -> shiguredo_video_device::Result<impl shiguredo_video_device::VideoDeviceList> {
    #[cfg(all(target_os = "linux", feature = "v4l2"))]
    return V4l2VideoDeviceList::enumerate();
    #[cfg(all(target_os = "linux", feature = "pipewire", not(feature = "v4l2")))]
    return PipewireVideoDeviceList::enumerate();
    #[cfg(target_os = "macos")]
    return AvfVideoDeviceList::enumerate();
    #[cfg(target_os = "windows")]
    return MfVideoDeviceList::enumerate();
}
```

注意: Linux で V4L2 と PipeWire の両方が有効な場合、サンプルでは V4L2 を優先する（`not(feature = "v4l2")` ガード）。ライブラリ自体は両バックエンドの型を同時に公開するため、利用者は `V4l2VideoDeviceList` と `PipewireVideoDeviceList` のどちらも直接使い分けることができる。

## テスト戦略

### 単体テスト

- `src/frame_math.rs` 内 `#[cfg(test)] mod tests`:
  - `nv12_plane_sizes`、`i420_plane_sizes`、`yuy2_packed_frame_bytes`（ストライドベース）のテスト（`capture.rs` から移行）
  - `nv12_packed_frame_bytes`、`i420_packed_frame_bytes`、`yuy2_packed_frame_bytes_win`（パックドバッファベース）のテストを新設
  - 境界値テスト: 0、負値、`i32::MAX` 付近でのオーバーフロー検出
- GAT ライフタイム制約のコンパイルテスト（`devices()` の結果がリストより長生きできないことをコンパイルエラーで検証）。`trybuild` は使わず `compile_fail` doctest で実現する。テスト対象は V4L2 バックエンド（Linux のみ有効だが、他プラットフォームでは型不在により vacuously pass するため許容）。例:

```rust
/// ```compile_fail
/// use shiguredo_video_device::{V4l2VideoDeviceList, VideoDeviceList};
/// let device = {
///     let list = V4l2VideoDeviceList::enumerate().unwrap();
///     &list.devices()[0] // list より長く device を借用しようとしてエラー
/// };
/// let _ = device;
/// ```
```

### 既存テストの移行

- `src/capture.rs` 内の `#[cfg(test)] mod tests`（`nv12_plane_sizes` 等のストライドベース関数テスト）は `src/frame_math.rs` 内の `#[cfg(test)] mod tests` に移行する
- `capture_windows.rs` の packed バージョン（`nv12_packed_frame_bytes` 等）にはテストが存在しないため、`src/frame_math.rs` への統合時に `#[cfg(test)] mod tests` にテストを新設する

## 対象ファイル

注意: `src/device.rs` と `src/capture.rs` は現在 `#[cfg(any(target_os = "macos", target_os = "linux"))]` で macOS と Linux の両方で使用されている。そのため「リネーム」ではなく「3 ファイルへの分割」となる。

| 変更対象 | 変更内容 |
|---|---|
| `src/lib.rs` | trait 定義の追加、モジュール宣言の更新、re-export の変更、crate-level doc comment の更新（`VideoCapture::new` 等のリンク修正） |
| `src/device.rs` → 削除・3 分割 | V4L2/PipeWire/macOS 共有だった実装を以下の 3 ファイルに分割 |
| `src/device_v4l2.rs` (新規) | V4L2 VideoDevice/VideoDeviceList の trait 実装（`ffi_v4l2` を直接呼出し） |
| `src/device_pipewire.rs` (新規) | PipeWire VideoDevice/VideoDeviceList の trait 実装（`ffi_pipewire` を直接呼出し） |
| `src/device_avf.rs` (新規) | macOS VideoDevice/VideoDeviceList の trait 実装（`ffi_avf` を直接呼出し） |
| `src/capture.rs` → 削除・3 分割 | V4L2/PipeWire/macOS 共有だった実装を以下の 3 ファイルに分割。フレーム計算関数は `frame_math.rs` に移動 |
| `src/capture_v4l2.rs` (新規) | V4L2 VideoCapture の trait 実装 + `extern "C" fn` コールバック（`ffi_v4l2` を直接呼出し） |
| `src/capture_pipewire.rs` (新規) | PipeWire VideoCapture の trait 実装 + `extern "C" fn` コールバック（`ffi_pipewire` を直接呼出し） |
| `src/capture_avf.rs` (新規) | macOS VideoCapture の trait 実装 + `extern "C" fn` コールバック（`ffi_avf` を直接呼出し） |
| `src/device_windows.rs` → `src/device_mf.rs` | Windows バックエンドのリネーム、trait 実装追加、`len()`/`is_empty()` 個別メソッド削除（trait デフォルト実装を使用） |
| `src/capture_windows.rs` → `src/capture_mf.rs` | Windows バックエンドのリネーム、`JoinHandle` 戻り値でコールバック回収する方式に変更、`MfShutdownGuard` 削除 |
| `src/ffi.rs` → 削除・3 分割 | `src/ffi_v4l2.rs` + `src/ffi_pipewire.rs` + `src/ffi_avf.rs` に分割 |
| `src/frame_math.rs` (新規) | フレームサイズ計算の共通関数（2 系統） |
| `src/types.rs` | `CoInitGuard` を `!Send` 化（`PhantomData<*const ()>` 追加）、`VIDEO_PIXEL_FORMAT_*` 定数と `from_raw`/`to_raw` の `#[cfg]` 削除、doc comment 更新 |
| `build.rs` | 相互排他 panic 削除、バックエンド別コンパイル・バインディング生成、`rerun-if-changed` 更新 |
| `Cargo.toml` | feature flag の相互排他制約撤廃 |
| `src/video_c.h` → `src/video_common.h` + `src/video_v4l2.h` + `src/video_pipewire.h` + `src/video_avf.h` | ヘッダ分割 |
| `src/video_v4l2.c` | 関数名に `video_v4l2_` 接頭辞を付加、`#include "video_c.h"` → `#include "video_v4l2.h"` |
| `src/video_pipewire.c` | 関数名に `video_pipewire_` 接頭辞を付加、`#include "video_c.h"` → `#include "video_pipewire.h"` |
| `src/video_c.m` → `src/video_avf.m` | リネーム、関数名に `video_avf_` 接頭辞を付加、`#include "video_c.h"` → `#include "video_avf.h"` |
| `examples/camera_preview.rs` | `use` 行の変更、`VideoCapture::new` → バックエンド別 `new`、`VideoDeviceList::enumerate()` → バックエンド別、`#[cfg]` 分岐の追加 |
| `examples/device_list.rs` | `for device in &device_list` → `for device in device_list.devices()`、`VideoDeviceList::enumerate()` → バックエンド別具象型 |
| `examples/device_info.rs` | `for device in &device_list`（2 箇所）→ `for device in device_list.devices()`、`VideoDeviceList::enumerate()` → バックエンド別具象型 |
| `CHANGES.md` | `## develop` に `[CHANGE]` エントリ（trait 化）、`[ADD]` エントリ（`to_raw` の全プラットフォーム公開）、`[FIX]` エントリ（Windows の再 start バグ修正）を追加 |
