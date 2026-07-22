# PipeWire のデバイス列挙がフォーマット情報を取得しない

- Priority: High
- Created: 2026-06-15
- Completed: 2026-06-17
- Model: Opus 4.7
- Branch: feature/fix-pipewire-enumerate-formats
- Polished: 2026-06-15

## 目的

`video_pipewire_enumerate_devices` (`src/video_pipewire.c:140-204`) は registry global で Node を見つけるとデバイス名と一意 ID のみを保存し、`device->formats = NULL; device->format_count = 0;` のまま列挙を完了する。結果として `VideoDeviceList::enumerate_pipewire()` から取得したデバイスの `formats()` が常に空となり、PipeWire バックエンドが「動作しているように見えて何も取れない」状態にある。各 Node プロキシに対して `SPA_PARAM_EnumFormat` を発行し、対応する SPA Video Raw フォーマット (`NV12` / `YUY2` / `I420`) を取得して `VideoDevice.formats` に集約する。

本 issue は第 1 段階として「対応フォーマット種別と (取得できる場合の) 解像度」を埋めることに集中し、fps の choice 展開と解像度 choice 展開は別 issue (refactor / change) で扱う。

## 優先度根拠

- High。`VideoDevice::formats()` は公開 API のひとつであり、PipeWire バックエンドだけ常に空配列を返すのは「動作しているように見えて何も取れない」状態
- README.md の対応プラットフォーム節で PipeWire は対応バックエンドとして案内されている。フォーマット列挙ゼロは契約違反
- `/review-code` の致命的指摘として確認された機能不全

## 現状

`src/video_pipewire.c:63-117` の `enum_registry_global` は Node を見つけたら `device->name` / `device->unique_id` を埋めて配列に push するだけで、フォーマット問い合わせを発行していない。L105-117 の該当ブロックを抜粋:

```c
struct VideoDevice* device = calloc(1, sizeof(struct VideoDevice));
if (!device) {
    return;
}
device->name = strdup(node_description ? node_description : node_name);
device->unique_id = strdup(node_name);
device->formats = NULL;
device->format_count = 0;   // フォーマット情報を一切取得しない

ctx->devices[ctx->count] = device;
ctx->count++;
```

`src/video_pipewire.c:125-132` の `enum_core_done` は最初の sync (`ctx->pending_sync`) 完了時点で `pw_main_loop_quit` を呼び、フォーマット取得を待たずに列挙ループを抜けてしまう。

実装方針としては、同ファイル L366-378 の `on_param_changed` (`SPA_PARAM_Format` の parse) と L251-262 の `convert_spa_video_format` をそのまま流用できる。

## 設計方針

### 1. registry global で Node プロキシをバインドして `SPA_PARAM_EnumFormat` を発行する

`EnumerateContext` (`src/video_pipewire.c:49-60`) に、Node ごとのフォーマット集約用配列を追加する。**既存フィールドはすべて維持** し、以下を新規追加する。

```c
struct PendingNode {
    int device_index;                        // ctx->devices 配列内のインデックス
    struct pw_proxy* node_proxy;             // pw_registry_bind の戻り値 (struct pw_proxy*)
    struct spa_hook node_listener;           // param イベントリスナ
    struct VideoFormatEntry* formats;        // 集約中の formats 配列
    int format_count;
    int format_capacity;
    int param_done;                          // この Node の param ストリーム終端を観測したか (next == 0)
};

struct EnumerateContext {
    // 既存フィールドはそのまま:
    struct pw_main_loop* loop;
    struct pw_context* context;
    struct pw_core* core;
    struct pw_registry* registry;
    struct spa_hook registry_listener;
    struct spa_hook core_listener;
    struct VideoDevice** devices;
    int count;
    int capacity;                            // devices と pending で共有する
    int pending_sync;                        // 1 段目 sync 用 (registry の global 列挙完了)
    // 追加フィールド:
    int enum_params_sync;                    // 2 段目 sync 用 (各 Node の enum_params 完了)
    struct PendingNode* pending;             // device と同じインデックスで Node 情報を保持
};
```

`enum_registry_global` (`src/video_pipewire.c:63-117`) では関数冒頭の `(void)id;` を削除し、Node を見つけて `ctx->devices` に push した直後に対応する `PendingNode` を初期化する。`ctx->devices` の配列拡張 (L93-102) と並行して `ctx->pending` も同じ `ctx->capacity` で `realloc` し、**`pending[i]` が常に `devices[i]` と同じ Node を指す不変条件** を維持する。`PendingNode` 用の独立した capacity は持たず、`ctx->capacity` を共有する。

配列拡張は両方とも `realloc` する必要があるため、片方成功・片方失敗時のメモリリークを防ぐために 1 つずつ順番に行い、後者が失敗したら直前の代入を巻き戻す:

```c
// devices と pending を同じ capacity で同期して拡張する
if (ctx->count >= ctx->capacity) {
    int new_capacity = ctx->capacity == 0 ? 8 : ctx->capacity * 2;
    struct VideoDevice** new_devices =
        realloc(ctx->devices, sizeof(struct VideoDevice*) * new_capacity);
    if (!new_devices) {
        return;
    }
    ctx->devices = new_devices;
    struct PendingNode* new_pending =
        realloc(ctx->pending, sizeof(struct PendingNode) * new_capacity);
    if (!new_pending) {
        // devices は拡張済みのまま (capacity は更新しない) で push を中止する。
        // 拡張済みメモリは ctx->devices として保持され、列挙関数末尾の cleanup で解放される
        return;
    }
    ctx->pending = new_pending;
    ctx->capacity = new_capacity;
}
```

`device` push 直後 (L116 `ctx->count++;` の **前**) に Node プロキシをバインドして `enum_params` を発行する。`pw_registry_bind` の `id` には `enum_registry_global` の `id` 引数 (registry のグローバル ID) をそのまま渡す。

```c
struct PendingNode* p = &ctx->pending[ctx->count];
memset(p, 0, sizeof(*p));
p->device_index = ctx->count;
p->node_proxy = pw_registry_bind(ctx->registry, id, PW_TYPE_INTERFACE_Node, PW_VERSION_NODE, 0);
if (!p->node_proxy) {
    // バインド失敗時は device を push せず return (calloc した device も free する)
    free(device->name);
    free(device->unique_id);
    free(device);
    return;
}
spa_zero(p->node_listener);
pw_node_add_listener((struct pw_node*)p->node_proxy, &p->node_listener, &enum_node_events, p);
pw_node_enum_params((struct pw_node*)p->node_proxy, 0, SPA_PARAM_EnumFormat, 0, UINT32_MAX, NULL);

ctx->devices[ctx->count] = device;
ctx->count++;
```

`pw_registry_bind` の戻り値は `void*` だが実体は `struct pw_proxy*`。`pw_node_*` API に渡す際は `(struct pw_node*)proxy` でキャストする。

### 2. `on_node_param` で SPA Video Raw フォーマットを parse する

実コード `src/video_pipewire.c:366-378` の `on_param_changed` と同じ API 使い方に揃える。`spa_format_parse` は `struct spa_video_info` を引数に取り、ラッパー型の `media_type` / `media_subtype` を埋める。`spa_format_video_raw_parse` は `info.info.raw` (ユニオン内の raw 構造体) を埋める。

```c
static void on_node_param(void* data, int seq, uint32_t id, uint32_t index, uint32_t next,
                          const struct spa_pod* param) {
    (void)seq;
    (void)index;
    struct PendingNode* p = data;

    if (id != SPA_PARAM_EnumFormat) {
        if (next == 0) {
            p->param_done = 1;
        }
        return;
    }
    if (!param) {
        if (next == 0) {
            p->param_done = 1;
        }
        return;
    }

    struct spa_video_info info;
    if (spa_format_parse(param, &info.media_type, &info.media_subtype) < 0) {
        goto check_end;
    }
    if (info.media_type != SPA_MEDIA_TYPE_video || info.media_subtype != SPA_MEDIA_SUBTYPE_raw) {
        goto check_end;
    }
    if (spa_format_video_raw_parse(param, &info.info.raw) < 0) {
        goto check_end;
    }

    uint32_t pixel_format = convert_spa_video_format(info.info.raw.format);
    if (pixel_format == 0) {
        // 未対応の SPA フォーマット (RGB 系・I422 等) は黙って skip する
        goto check_end;
    }

    // info.info.raw.size / framerate が choice 形式の場合は単一値を取り出せない可能性がある。
    // 第 1 段階では「単一値が parse できた場合のみ採用」とし、choice 展開は別 issue で扱う。
    int width = info.info.raw.size.width;
    int height = info.info.raw.size.height;
    if (width <= 0 || height <= 0) {
        goto check_end;
    }

    // p->formats 配列を必要なら拡張し、新フォーマットを push する
    if (p->format_count >= p->format_capacity) {
        int new_cap = p->format_capacity == 0 ? 4 : p->format_capacity * 2;
        struct VideoFormatEntry* new_formats = realloc(p->formats, sizeof(*new_formats) * new_cap);
        if (!new_formats) {
            goto check_end;
        }
        p->formats = new_formats;
        p->format_capacity = new_cap;
    }
    p->formats[p->format_count].width = width;
    p->formats[p->format_count].height = height;
    p->formats[p->format_count].pixel_format = pixel_format;
    // fps は本 issue では未取得のため仮値 (min=1.0, max=30.0) を埋める。
    // - 0.0 は CI の Device Test ジョブが `all(.max_fps > 0)` を要求しているため使えない
    //   (.github/workflows/ci.yml の device_info jq 検証参照)
    // - NaN は jq / JSON 出力で扱いづらい
    // - 仮値 (1.0, 30.0) は実用的なデフォルトで CI の jq 検証も通る
    // 正確な fps 抽出 (choice 形式の展開) は別 issue で扱う
    p->formats[p->format_count].min_fps = 1.0f;
    p->formats[p->format_count].max_fps = 30.0f;
    p->format_count++;

check_end:
    if (next == 0) {
        p->param_done = 1;
    }
}

static const struct pw_node_events enum_node_events = {
    PW_VERSION_NODE_EVENTS,
    .param = on_node_param,
};
```

choice 形式 (`SPA_TYPE_Choice` の `SPA_CHOICE_Range` / `SPA_CHOICE_Enum`) の展開はスコープ外。第 1 段階では単一値が取れる Node のみフォーマットが埋まる。choice 形式しか提供しない Node では `format_count == 0` のままになるが、それでもデバイスはリストに残す (利用者が「対応フォーマット情報なし」と判断可能)。

### 3. 完了判定を 2 段階 sync で行う

`SPA_PARAM_EnumFormat` の param イベントは `pw_core_sync` の done より後に来うる。デッドロックを避けるために 2 段階 sync を採用する。

1. 1 段目 sync (`ctx->pending_sync`、現状の実装と同じ): registry の global 列挙が完了した時点で done が来る。done のなかで全 `PendingNode` に対して **すでに `enum_params` を発行済み** であることを確認し、2 段目 sync (`ctx->enum_params_sync = pw_core_sync(ctx->core, PW_ID_CORE, 0)`) を発行する
2. 2 段目 sync (`ctx->enum_params_sync`): 2 段目 sync の done が来た時点で、全 Node の param ストリームが終端していることが保証される。`pw_main_loop_quit` を呼ぶ

```c
static void enum_core_done(void* data, uint32_t id, int seq) {
    struct EnumerateContext* ctx = data;
    (void)id;

    if (seq == ctx->pending_sync) {
        // 1 段目: registry の global 列挙完了。enum_params は既に発行済みなので、
        // 結果が出揃うのを待つために 2 段目 sync を発行する
        ctx->enum_params_sync = pw_core_sync(ctx->core, PW_ID_CORE, 0);
        return;
    }
    if (seq == ctx->enum_params_sync) {
        // 2 段目: 全 Node の param ストリームが終端しているはず
        pw_main_loop_quit(ctx->loop);
    }
}
```

`on_node_param` の `next == 0` で `param_done = 1` を立てるロジックは保険として残す。`enum_core_done` の 2 段目では `param_done` を直接見ずに quit する (PipeWire の sync 順序保証に従う)。

### 4. クリーンアップと結果の `VideoDevice.formats` へのコピー

`pw_main_loop_quit` 後、`video_pipewire_enumerate_devices` の cleanup ブロック (`src/video_pipewire.c:193-199` 周辺) で:

- 各 `PendingNode` について `spa_hook_remove(&p->node_listener);` と `pw_proxy_destroy(p->node_proxy);` を呼ぶ
- `p->formats` を `ctx->devices[p->device_index]->formats` に move (`device->formats = p->formats; device->format_count = p->format_count;`)。`PendingNode` 側は所有権を手放したのでポインタを NULL 化する
- `ctx->pending` 配列自体を `free(ctx->pending)` で解放する

これを行ったあとに既存の `pw_proxy_destroy((struct pw_proxy*)ctx.registry);` 〜 `pw_main_loop_destroy(ctx.loop);` が続く。

## 影響範囲

- `src/video_pipewire.c`: `EnumerateContext` 拡張、`enum_registry_global` 改修、`on_node_param` および `enum_node_events` 新規追加、`enum_core_done` 改修、`video_pipewire_enumerate_devices` の cleanup ブロックに `PendingNode` 後処理を追加
- `src/video_pipewire.h`, `src/video.h`, `src/capture_ffi.rs`, `src/device_ffi.rs`: 変更なし (公開 ABI は不変、`VideoFormatEntry` は既存型を流用)
- `examples/device_info.rs`: 変更なし。本修正により `PipeWire` バックエンドのデバイスの `format_count` が 0 でなくなる
- 0024 (`pw_init` / `pw_deinit` のバランス) と同じ `src/video_pipewire.c` を触る。0024 を先にマージし、本 issue はその上にリベースする方針 (0024 側の影響範囲セクションでも同方針が明示されている)。コンフリクトが起きた場合は手動でマージする

## 完了条件

- `video_pipewire_enumerate_devices` が PipeWire `Video/Source` Node ごとの対応フォーマット (`SPA_VIDEO_FORMAT_NV12 / YUY2 / I420`) を取得し、`VideoDevice.formats` に格納する
- 取得結果が `VideoDevice::formats()` 経由でユーザに見える
- fps は本 issue では未取得を表す仮値 (`min_fps = 1.0`, `max_fps = 30.0`) で埋めること。choice 形式の展開と正確な fps 抽出は本 issue では行わない (別 issue で対応)。`0.0` は使わない (`.github/workflows/ci.yml` の Device Test ジョブが `.devices | all(.formats | all(.max_fps > 0))` を要求しているため、`0.0` だと CI が失敗する)
- 単一値が取れない Node (解像度・fps とも choice 形式しか提供しない Node) では `format_count == 0` を許容する (デバイス自体はリストに残す)。ただし `.github/workflows/ci.yml` の Device Test ジョブは `.devices | all(.format_count > 0)` を要求しているため、self-hosted Linux runner のカメラが choice 形式しか返さない場合は Device Test が落ちる可能性がある。その場合は本 issue の修正で対処せず、choice 展開を扱う後続 issue で解消する
- 2 段階 sync で param ストリームの完了を待つこと。1 段階 sync のままだとデッドロックまたは取りこぼしが発生する
- `PendingNode` ごとに `pw_proxy_destroy` および `spa_hook_remove` を漏れなく呼び、`ctx->pending` 配列を `free` する
- 既存 V4L2 経路には影響しない
- CI 検証: 本リポジトリの `.github/workflows/ci.yml` の `Ubuntu (pipewire)` および `Ubuntu (v4l2,pipewire)` ジョブで `cargo test --workspace` および `cargo build --examples` が通る。実機キャプチャを伴う動作確認は Device Test ワークフロー (`name: Linux`) の手動実行で行う (CI ジョブにカメラが接続されていない場合は format 取得を伴わないビルド・clippy 検証のみとなることを許容する)
- `cargo clippy --workspace --all-targets -- -D warnings` および C 側ビルド警告 (`-Wall -Wextra` 相当) 0 件
- `CHANGES.md` の `## develop` 配下に `[FIX]` エントリを追加する (例: `[FIX] PipeWire でデバイスのフォーマット列挙が常に空だったのを修正する`)。「fps は本第 1 段階では仮値 (1.0 / 30.0) で埋まり、choice 形式の正確な fps 抽出は別途対応」旨を 1 行注記する。担当者行 (`- @<github-id>`) を含める
- `pw_node_add_listener` および `pw_node_enum_params` の戻り値 (負値で失敗) はチェックしない。失敗した場合はその Node の `format_count` が 0 のままになり、リストには残るが対応フォーマット情報なしと扱われる。これは scope 外の対応 (失敗時の挙動を改善するなら別 issue) で許容する

## スコープ外

- fps の choice 形式 (`SPA_CHOICE_Range` / `SPA_CHOICE_Enum`) の展開と正確な min/max 抽出
- 解像度の choice 形式の展開 (現状は単一値のみ採用)
- PipeWire 経由の MJPEG / H.264 等圧縮フォーマット対応
- フォーマット情報を非同期に再取得する hot-plug 対応
- README.md / docs/LINUX.md への記述追加 (公開挙動 = `formats()` が空でなくなる、は CHANGES.md エントリで十分。README / docs は別 doc issue で扱う)

## 解決方法

### フォーマット列挙修正

`video_pipewire_enumerate_devices` がフォーマット情報を一切取得せず `formats = NULL; format_count = 0` のままだった問題を修正した。

- `PendingNode` 構造体を追加し、`EnumerateContext` に `pending` 配列と `enum_params_sync` フィールドを拡張
- `enum_registry_global` で Video/Source Node 発見時に `pw_registry_bind` → `pw_node_add_listener` → `pw_node_enum_params(SPA_PARAM_EnumFormat)` を発行
- `on_node_param` コールバックで `spa_format_parse` / `spa_format_video_raw_parse` / `convert_spa_video_format` により NV12 / YUY2 / I420 を抽出
- `enum_core_done` を 2 段階 sync 化。1 段目で registry 列挙完了後、2 段目 sync で全 Node の param ストリーム完了を待ってから quit
- クリーンアップで `PendingNode.formats` を `VideoDevice.formats` に移動し `pending` 配列を解放
- fps は未取得のため仮値 (1.0 / 30.0) を設定。choice 展開は本修正のスコープ外
- `tests/test_pipewire.rs` を新規追加

### キャプチャ開始時の無限ループ修正

`video_pipewire_session_start` の while ループが `pw_stream_get_state` で `PW_STREAM_STATE_ERROR` を検出できず無限ループする問題を修正した（PipeWire 内部で ERROR 状態が PAUSED にリセットされるため）。

- `VideoSession` に `atomic_int stream_error` フラグを追加
- `on_stream_state_changed` で ERROR 遷移時にフラグを立てる
- while ループで `stream_error` フラグと `pw_stream_get_state` のエラーメッセージの両方をチェックし、エラー時は即座に `-5` で復帰

### フォーマット交渉の改善

`spa_format_video_raw_build` による単一値指定では V4L2 プラグインとのフォーマット交渉に失敗しキャプチャが開始できない問題を修正した。

- フォーマット指定を `spa_pod_builder_add_object` による手動構築に変更
- `SPA_FORMAT_VIDEO_format`: NV12 / YUY2 / I420 の choice 列挙
- `SPA_FORMAT_VIDEO_framerate`: choice range (default=リクエスト値, min=1fps, max=60fps)
- `SPA_FORMAT_VIDEO_size`: choice range (default=リクエスト値, min=1x1, max=3840x2160)

### PipeWire 用 CI 整備

Device Test ジョブで PipeWire バックエンドのテストが実行できるよう CI を整備した。

- Device Test ジョブに `systemctl --user start pipewire` を追加し、PipeWire デーモンを起動
- self-hosted Linux runner 向けに `XDG_RUNTIME_DIR` 設定を追加
- Windows clippy 対応と CI ステップの環境ごとの分離
