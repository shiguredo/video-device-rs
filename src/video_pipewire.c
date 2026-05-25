// PipeWire を使った Linux 用ビデオキャプチャ実装

#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>
#include <spa/debug/types.h>
#include <spa/pod/builder.h>

#include "video_c.h"

// VideoDevice 構造体
struct VideoDevice {
    char* name;
    char* unique_id;
    struct VideoFormatEntry* formats;
    int format_count;
};

// VideoSession 構造体
struct VideoSession {
    struct pw_thread_loop* thread_loop;
    struct pw_context* context;
    struct pw_core* core;
    struct pw_stream* stream;
    struct spa_hook core_listener;
    struct spa_hook stream_listener;
    char* device_id;
    FrameCallback callback;
    void* user_data;
    int requested_width;
    int requested_height;
    int requested_fps;
    uint32_t requested_format;
    int negotiated_width;
    int negotiated_height;
    uint32_t negotiated_format;
    int negotiated_stride;
    atomic_int running;
    atomic_int format_ready;
};

// デバイス列挙用のコンテキスト
struct EnumerateContext {
    struct pw_main_loop* loop;
    struct pw_context* context;
    struct pw_core* core;
    struct pw_registry* registry;
    struct spa_hook registry_listener;
    struct spa_hook core_listener;
    struct VideoDevice** devices;
    int count;
    int capacity;
    int pending_sync;
};

// デバイス列挙: registry global イベント
static void enum_registry_global(void* data, uint32_t id,
                                 uint32_t permissions, const char* type,
                                 uint32_t version,
                                 const struct spa_dict* props) {
    (void)id;
    (void)permissions;
    (void)version;

    struct EnumerateContext* ctx = data;

    if (strcmp(type, PW_TYPE_INTERFACE_Node) != 0) {
        return;
    }

    if (!props) {
        return;
    }

    // Video/Source のみを対象にする
    const char* media_class = spa_dict_lookup(props, PW_KEY_MEDIA_CLASS);
    if (!media_class || strcmp(media_class, "Video/Source") != 0) {
        return;
    }

    const char* node_name = spa_dict_lookup(props, PW_KEY_NODE_NAME);
    const char* node_description = spa_dict_lookup(props, PW_KEY_NODE_DESCRIPTION);
    if (!node_name) {
        return;
    }

    // 配列を拡張する
    if (ctx->count >= ctx->capacity) {
        int new_capacity = ctx->capacity == 0 ? 8 : ctx->capacity * 2;
        struct VideoDevice** new_devices =
            realloc(ctx->devices, sizeof(struct VideoDevice*) * new_capacity);
        if (!new_devices) {
            return;
        }
        ctx->devices = new_devices;
        ctx->capacity = new_capacity;
    }

    struct VideoDevice* device = calloc(1, sizeof(struct VideoDevice));
    if (!device) {
        return;
    }

    device->name = strdup(node_description ? node_description : node_name);
    device->unique_id = strdup(node_name);
    device->formats = NULL;
    device->format_count = 0;

    ctx->devices[ctx->count] = device;
    ctx->count++;
}

static const struct pw_registry_events enum_registry_events = {
    PW_VERSION_REGISTRY_EVENTS,
    .global = enum_registry_global,
};

// デバイス列挙: core done イベント (sync 完了)
static void enum_core_done(void* data, uint32_t id, int seq) {
    struct EnumerateContext* ctx = data;
    (void)id;

    if (seq == ctx->pending_sync) {
        pw_main_loop_quit(ctx->loop);
    }
}

static const struct pw_core_events enum_core_events = {
    PW_VERSION_CORE_EVENTS,
    .done = enum_core_done,
};

// デバイス列挙
int video_enumerate_devices(struct VideoDevice*** devices, int* count) {
    if (!devices || !count) {
        return -1;
    }

    *devices = NULL;
    *count = 0;

    pw_init(NULL, NULL);

    struct EnumerateContext ctx = {0};

    ctx.loop = pw_main_loop_new(NULL);
    if (!ctx.loop) {
        return -2;
    }

    ctx.context =
        pw_context_new(pw_main_loop_get_loop(ctx.loop), NULL, 0);
    if (!ctx.context) {
        pw_main_loop_destroy(ctx.loop);
        return -2;
    }

    ctx.core = pw_context_connect(ctx.context, NULL, 0);
    if (!ctx.core) {
        pw_context_destroy(ctx.context);
        pw_main_loop_destroy(ctx.loop);
        return -3;
    }

    // core の done イベントを監視する
    spa_zero(ctx.core_listener);
    pw_core_add_listener(ctx.core, &ctx.core_listener, &enum_core_events,
                         &ctx);

    ctx.registry = pw_core_get_registry(ctx.core, PW_VERSION_REGISTRY, 0);
    if (!ctx.registry) {
        pw_core_disconnect(ctx.core);
        pw_context_destroy(ctx.context);
        pw_main_loop_destroy(ctx.loop);
        return -4;
    }

    spa_zero(ctx.registry_listener);
    pw_registry_add_listener(ctx.registry, &ctx.registry_listener,
                             &enum_registry_events, &ctx);

    // sync を送って全 global の列挙完了を待つ
    ctx.pending_sync = pw_core_sync(ctx.core, PW_ID_CORE, 0);

    pw_main_loop_run(ctx.loop);

    // クリーンアップ
    spa_hook_remove(&ctx.registry_listener);
    pw_proxy_destroy((struct pw_proxy*)ctx.registry);
    spa_hook_remove(&ctx.core_listener);
    pw_core_disconnect(ctx.core);
    pw_context_destroy(ctx.context);
    pw_main_loop_destroy(ctx.loop);

    *devices = ctx.devices;
    *count = ctx.count;
    return 0;
}

void video_free_devices(struct VideoDevice** devices, int count) {
    if (!devices) {
        return;
    }

    for (int i = 0; i < count; i++) {
        if (devices[i]) {
            free(devices[i]->name);
            free(devices[i]->unique_id);
            free(devices[i]->formats);
            free(devices[i]);
        }
    }
    free(devices);
}

const char* video_device_name(struct VideoDevice* device) {
    if (!device) {
        return NULL;
    }
    return device->name;
}

const char* video_device_unique_id(struct VideoDevice* device) {
    if (!device) {
        return NULL;
    }
    return device->unique_id;
}

int video_device_format_count(struct VideoDevice* device) {
    if (!device) {
        return 0;
    }
    return device->format_count;
}

const struct VideoFormatEntry* video_device_get_format(struct VideoDevice* device, int index) {
    if (!device || index < 0 || index >= device->format_count) {
        return NULL;
    }
    return &device->formats[index];
}

// SPA ビデオフォーマットを VIDEO_PIXEL_FORMAT_* に変換
static uint32_t convert_spa_video_format(uint32_t spa_format) {
    switch (spa_format) {
        case SPA_VIDEO_FORMAT_NV12:
            return VIDEO_PIXEL_FORMAT_NV12;
        case SPA_VIDEO_FORMAT_YUY2:
            return VIDEO_PIXEL_FORMAT_YUY2;
        case SPA_VIDEO_FORMAT_I420:
            return VIDEO_PIXEL_FORMAT_I420;
        default:
            return 0;
    }
}

static uint32_t convert_video_pixel_format_to_spa(uint32_t pixel_format) {
    switch (pixel_format) {
        case VIDEO_PIXEL_FORMAT_NV12:
            return SPA_VIDEO_FORMAT_NV12;
        case VIDEO_PIXEL_FORMAT_YUY2:
            return SPA_VIDEO_FORMAT_YUY2;
        case VIDEO_PIXEL_FORMAT_I420:
            return SPA_VIDEO_FORMAT_I420;
        default:
            return SPA_VIDEO_FORMAT_UNKNOWN;
    }
}

// uint64_t の乗算でオーバーフローしないか確認する
static int mul_u64_ov(uint64_t a, uint64_t b, uint64_t* out) {
    if (a != 0 && b > UINT64_MAX / a) {
        return 0;
    }
    *out = a * b;
    return 1;
}

// uint64_t の加算でオーバーフローしないか確認する
static int add_u64_ov(uint64_t a, uint64_t b, uint64_t* out) {
    if (a > UINT64_MAX - b) {
        return 0;
    }
    *out = a + b;
    return 1;
}

// NV12: Rust `nv12_plane_sizes`（capture.rs）と同じ Y/UV バイト数
static int required_bytes_nv12(int32_t stride, int32_t stride_uv, int height, uint64_t* out) {
    if (stride <= 0 || stride_uv <= 0 || height <= 0) {
        return 0;
    }
    uint64_t y_bytes;
    if (!mul_u64_ov((uint64_t)stride, (uint64_t)height, &y_bytes)) {
        return 0;
    }
    // UV 行数は `height.div_ceil(2)` と同じ（奇数高さで `height/2` より 1 行多い）
    uint64_t uv_h = ((uint64_t)height + 1u) / 2u;
    uint64_t uv_bytes;
    if (!mul_u64_ov((uint64_t)stride_uv, uv_h, &uv_bytes)) {
        return 0;
    }
    return add_u64_ov(y_bytes, uv_bytes, out);
}

// I420: Rust `i420_plane_sizes`（capture.rs）と同じ Y/連結 UV バイト数
static int required_bytes_i420(int32_t stride, int32_t stride_uv, int height, uint64_t* out) {
    if (stride <= 0 || stride_uv <= 0 || height <= 0) {
        return 0;
    }
    uint64_t y_bytes;
    if (!mul_u64_ov((uint64_t)stride, (uint64_t)height, &y_bytes)) {
        return 0;
    }
    uint64_t chroma_h = ((uint64_t)height + 1u) / 2u;
    uint64_t uv_part;
    if (!mul_u64_ov((uint64_t)stride_uv, chroma_h, &uv_part)) {
        return 0;
    }
    uint64_t uv_bytes;
    if (!mul_u64_ov(uv_part, 2u, &uv_bytes)) {
        return 0;
    }
    return add_u64_ov(y_bytes, uv_bytes, out);
}

// YUY2: 最終行の末尾まで含めた必要バイト数（stride >= width*2 を満たさない場合は不正とみなす）
static int required_bytes_yuy2(int32_t stride, int width, int height, uint64_t* out) {
    if (stride <= 0 || width <= 0 || height <= 0) {
        return 0;
    }
    uint64_t w2;
    if (!mul_u64_ov((uint64_t)width, 2u, &w2)) {
        return 0;
    }
    if ((uint64_t)stride < w2) {
        return 0;
    }
    uint64_t s = (uint64_t)stride;
    uint64_t h = (uint64_t)height;
    uint64_t rows = 0;
    if (h > 1u) {
        if (!mul_u64_ov(s, h - 1u, &rows)) {
            return 0;
        }
    }
    return add_u64_ov(rows, w2, out);
}

// ストリームの param_changed コールバック
static void on_param_changed(void* userdata, uint32_t id,
                              const struct spa_pod* param) {
    struct VideoSession* session = userdata;

    if (id != SPA_PARAM_Format || !param) {
        return;
    }

    struct spa_video_info info;
    if (spa_format_parse(param, &info.media_type, &info.media_subtype) < 0) {
        return;
    }

    if (info.media_type != SPA_MEDIA_TYPE_video ||
        info.media_subtype != SPA_MEDIA_SUBTYPE_raw) {
        return;
    }

    if (spa_format_video_raw_parse(param, &info.info.raw) < 0) {
        return;
    }

    session->negotiated_width = info.info.raw.size.width;
    session->negotiated_height = info.info.raw.size.height;
    session->negotiated_format = convert_spa_video_format(info.info.raw.format);
    atomic_store(&session->format_ready, 1);
}

// ストリームの process コールバック
static void on_process(void* userdata) {
    struct VideoSession* session = userdata;

    if (!atomic_load(&session->running)) {
        return;
    }

    if (!atomic_load(&session->format_ready)) {
        return;
    }

    struct pw_buffer* buf = pw_stream_dequeue_buffer(session->stream);
    if (!buf) {
        return;
    }

    struct spa_buffer* spa_buf = buf->buffer;
    if (spa_buf->n_datas == 0 || !spa_buf->datas[0].data) {
        pw_stream_queue_buffer(session->stream, buf);
        return;
    }

    // chunk が NULL のとき stride を参照すると未定義動作になる
    if (!spa_buf->datas[0].chunk) {
        pw_stream_queue_buffer(session->stream, buf);
        return;
    }

    const struct spa_chunk* chunk = spa_buf->datas[0].chunk;
    int32_t stride = chunk->stride;
    // 以降の y_size 計算で負や 0 の stride は使わない
    if (stride <= 0) {
        pw_stream_queue_buffer(session->stream, buf);
        return;
    }

    const uint8_t* data = spa_buf->datas[0].data;

    // タイムスタンプを取得する
    int64_t timestamp_us;
    struct spa_meta_header* header =
        spa_buffer_find_meta_data(spa_buf, SPA_META_Header,
                                  sizeof(struct spa_meta_header));
    if (header && header->pts != (int64_t)INT64_MIN) {
        // PipeWire のタイムスタンプは ns 単位
        timestamp_us = header->pts / 1000;
    } else {
        struct timespec ts;
        clock_gettime(CLOCK_MONOTONIC, &ts);
        timestamp_us = (int64_t)ts.tv_sec * 1000000 + ts.tv_nsec / 1000;
    }

    if (session->callback) {
        uint32_t format = session->negotiated_format;
        int width = session->negotiated_width;
        int height = session->negotiated_height;

        if (width <= 0 || height <= 0) {
            pw_stream_queue_buffer(session->stream, buf);
            return;
        }

        uint64_t need = 0;
        if (format == VIDEO_PIXEL_FORMAT_NV12) {
            if (!required_bytes_nv12(stride, stride, height, &need)) {
                pw_stream_queue_buffer(session->stream, buf);
                return;
            }
        } else if (format == VIDEO_PIXEL_FORMAT_I420) {
            int stride_uv = (stride + 1) / 2;
            if (!required_bytes_i420(stride, stride_uv, height, &need)) {
                pw_stream_queue_buffer(session->stream, buf);
                return;
            }
        } else if (format == VIDEO_PIXEL_FORMAT_YUY2) {
            if (!required_bytes_yuy2(stride, width, height, &need)) {
                pw_stream_queue_buffer(session->stream, buf);
                return;
            }
        } else {
            pw_stream_queue_buffer(session->stream, buf);
            return;
        }

        // 壊れたメタデータで offset や size が不正な場合にバッファ外読みを防ぐ
        uint32_t maxsize = spa_buf->datas[0].maxsize;
        if (chunk->offset > maxsize || need > maxsize - chunk->offset) {
            pw_stream_queue_buffer(session->stream, buf);
            return;
        }
        if (chunk->size == 0 || need > (uint64_t)chunk->size) {
            pw_stream_queue_buffer(session->stream, buf);
            return;
        }

        // offset と size の検証を通過してからポインタ演算する
        const uint8_t* plane = data + chunk->offset;

        if (format == VIDEO_PIXEL_FORMAT_NV12) {
            // NV12: Y プレーンと UV プレーンが連続
            uint64_t y_bytes = (uint64_t)stride * (uint64_t)height;
            const uint8_t* uv_data = plane + y_bytes;
            int stride_uv = stride;

            session->callback(session->user_data, plane, uv_data, width,
                              height, stride, stride_uv,
                              VIDEO_PIXEL_FORMAT_NV12, timestamp_us, NULL);
        } else if (format == VIDEO_PIXEL_FORMAT_YUY2) {
            // YUY2: パックドフォーマット
            session->callback(session->user_data, plane, NULL, width,
                              height, stride, 0,
                              VIDEO_PIXEL_FORMAT_YUY2, timestamp_us, NULL);
        } else if (format == VIDEO_PIXEL_FORMAT_I420) {
            // I420: Y, U, V が連続
            uint64_t y_bytes = (uint64_t)stride * (uint64_t)height;
            const uint8_t* uv_data = plane + y_bytes;
            int stride_uv = (stride + 1) / 2;

            session->callback(session->user_data, plane, uv_data, width,
                              height, stride, stride_uv,
                              VIDEO_PIXEL_FORMAT_I420, timestamp_us, NULL);
        }
    }

    pw_stream_queue_buffer(session->stream, buf);
}

// ストリーム状態変更コールバック
static void on_stream_state_changed(void* userdata,
                                    enum pw_stream_state old,
                                    enum pw_stream_state state,
                                    const char* error) {
    (void)old;
    (void)error;

    struct VideoSession* session = userdata;

    switch (state) {
        case PW_STREAM_STATE_STREAMING:
        case PW_STREAM_STATE_ERROR:
        case PW_STREAM_STATE_UNCONNECTED:
            pw_thread_loop_signal(session->thread_loop, false);
            break;
        default:
            break;
    }
}

static const struct pw_stream_events stream_events = {
    PW_VERSION_STREAM_EVENTS,
    .process = on_process,
    .state_changed = on_stream_state_changed,
    .param_changed = on_param_changed,
};

// core の done イベント (セッション接続用)
static void session_core_done(void* data, uint32_t id, int seq) {
    (void)id;
    (void)seq;
    struct VideoSession* session = data;
    pw_thread_loop_signal(session->thread_loop, false);
}

// core の error イベント
static void session_core_error(void* data, uint32_t id, int seq, int res,
                               const char* message) {
    (void)id;
    (void)seq;
    (void)res;
    (void)message;
    struct VideoSession* session = data;
    pw_thread_loop_signal(session->thread_loop, false);
}

static const struct pw_core_events session_core_events = {
    PW_VERSION_CORE_EVENTS,
    .done = session_core_done,
    .error = session_core_error,
};

struct VideoSession* video_session_create(const char* device_id, int width,
                                          int height, int fps,
                                          uint32_t requested_pixel_format) {
    struct VideoSession* session = calloc(1, sizeof(struct VideoSession));
    if (!session) {
        return NULL;
    }

    pw_init(NULL, NULL);

    // デフォルト値の設定
    if (width <= 0) {
        width = 640;
    }
    if (height <= 0) {
        height = 480;
    }
    if (fps <= 0) {
        fps = 30;
    }

    session->requested_format =
        requested_pixel_format == 0
            ? SPA_VIDEO_FORMAT_NV12
            : convert_video_pixel_format_to_spa(requested_pixel_format);
    if (session->requested_format == SPA_VIDEO_FORMAT_UNKNOWN) {
        free(session);
        return NULL;
    }

    session->requested_width = width;
    session->requested_height = height;
    session->requested_fps = fps;
    session->device_id = device_id ? strdup(device_id) : NULL;
    atomic_init(&session->running, 0);
    atomic_init(&session->format_ready, 0);

    // thread loop を作成する
    session->thread_loop = pw_thread_loop_new("shiguredo-video", NULL);
    if (!session->thread_loop) {
        free(session->device_id);
        free(session);
        return NULL;
    }

    session->context = pw_context_new(
        pw_thread_loop_get_loop(session->thread_loop), NULL, 0);
    if (!session->context) {
        pw_thread_loop_destroy(session->thread_loop);
        free(session->device_id);
        free(session);
        return NULL;
    }

    return session;
}

void video_session_destroy(struct VideoSession* session) {
    if (!session) {
        return;
    }

    if (atomic_load(&session->running)) {
        video_session_stop(session);
    }

    if (session->stream) {
        pw_stream_destroy(session->stream);
    }

    if (session->core) {
        spa_hook_remove(&session->core_listener);
        pw_core_disconnect(session->core);
    }

    if (session->context) {
        pw_context_destroy(session->context);
    }

    if (session->thread_loop) {
        pw_thread_loop_destroy(session->thread_loop);
    }

    free(session->device_id);
    free(session);
}

int video_session_start(struct VideoSession* session, FrameCallback callback,
                        void* user_data) {
    if (!session || !callback) {
        return -1;
    }

    if (atomic_load(&session->running)) {
        return 0;
    }

    session->callback = callback;
    session->user_data = user_data;

    // thread loop を開始する
    if (pw_thread_loop_start(session->thread_loop) < 0) {
        return -2;
    }

    pw_thread_loop_lock(session->thread_loop);

    // PipeWire に接続する
    session->core = pw_context_connect(session->context, NULL, 0);
    if (!session->core) {
        pw_thread_loop_unlock(session->thread_loop);
        pw_thread_loop_stop(session->thread_loop);
        return -3;
    }

    spa_zero(session->core_listener);
    pw_core_add_listener(session->core, &session->core_listener,
                         &session_core_events, session);

    // ストリームのプロパティを設定する
    struct pw_properties* props =
        pw_properties_new(PW_KEY_MEDIA_TYPE, "Video",
                          PW_KEY_MEDIA_CATEGORY, "Capture",
                          PW_KEY_MEDIA_ROLE, "Communication", NULL);

    if (!props) {
        pw_core_disconnect(session->core);
        session->core = NULL;
        pw_thread_loop_unlock(session->thread_loop);
        pw_thread_loop_stop(session->thread_loop);
        return -4;
    }

    if (session->device_id) {
        pw_properties_set(props, PW_KEY_TARGET_OBJECT,
                          session->device_id);
    }

    // ストリームを作成する
    session->stream =
        pw_stream_new(session->core, "video-capture", props);
    if (!session->stream) {
        pw_core_disconnect(session->core);
        session->core = NULL;
        pw_thread_loop_unlock(session->thread_loop);
        pw_thread_loop_stop(session->thread_loop);
        return -4;
    }

    spa_zero(session->stream_listener);
    pw_stream_add_listener(session->stream, &session->stream_listener,
                           &stream_events, session);

    // ビデオフォーマットを設定する
    uint8_t params_buffer[1024];
    struct spa_pod_builder builder =
        SPA_POD_BUILDER_INIT(params_buffer, sizeof(params_buffer));

    struct spa_video_info_raw video_info = {0};
    video_info.format = session->requested_format;
    video_info.size.width = session->requested_width;
    video_info.size.height = session->requested_height;
    video_info.framerate.num = session->requested_fps;
    video_info.framerate.denom = 1;

    const struct spa_pod* params[1];
    params[0] = spa_format_video_raw_build(&builder, SPA_PARAM_EnumFormat,
                                           &video_info);

    // ストリームを接続する
    int result = pw_stream_connect(
        session->stream, PW_DIRECTION_INPUT, PW_ID_ANY,
        PW_STREAM_FLAG_AUTOCONNECT | PW_STREAM_FLAG_MAP_BUFFERS,
        params, 1);

    if (result < 0) {
        pw_stream_destroy(session->stream);
        session->stream = NULL;
        spa_hook_remove(&session->core_listener);
        pw_core_disconnect(session->core);
        session->core = NULL;
        pw_thread_loop_unlock(session->thread_loop);
        pw_thread_loop_stop(session->thread_loop);
        return -5;
    }

    // ストリームが streaming 状態になるまで待機する
    while (1) {
        enum pw_stream_state state = pw_stream_get_state(
            session->stream, NULL);
        if (state == PW_STREAM_STATE_STREAMING) {
            break;
        }
        if (state == PW_STREAM_STATE_ERROR ||
            state == PW_STREAM_STATE_UNCONNECTED) {
            pw_stream_destroy(session->stream);
            session->stream = NULL;
            spa_hook_remove(&session->core_listener);
            pw_core_disconnect(session->core);
            session->core = NULL;
            pw_thread_loop_unlock(session->thread_loop);
            pw_thread_loop_stop(session->thread_loop);
            return -5;
        }
        pw_thread_loop_wait(session->thread_loop);
    }

    atomic_store(&session->running, 1);
    pw_thread_loop_unlock(session->thread_loop);

    return 0;
}

void video_session_stop(struct VideoSession* session) {
    if (!session || !atomic_load(&session->running)) {
        return;
    }

    atomic_store(&session->running, 0);

    pw_thread_loop_lock(session->thread_loop);

    if (session->stream) {
        pw_stream_disconnect(session->stream);
    }

    pw_thread_loop_unlock(session->thread_loop);

    pw_thread_loop_stop(session->thread_loop);
}
