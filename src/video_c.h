#pragma once

#include <stdint.h>

#if defined(__cplusplus)
extern "C" {
#endif

struct VideoDevice;
struct VideoSession;

// ピクセルフォーマット定数
#define VIDEO_PIXEL_FORMAT_NV12 0x3231564E  // '420v' (NV12)
#define VIDEO_PIXEL_FORMAT_YUY2 0x32595559  // 'yuvs' (YUY2)
#define VIDEO_PIXEL_FORMAT_I420 0x30323449  // 'I420'

// フォーマットエントリ
struct VideoFormatEntry {
    int width;
    int height;
    float min_fps;
    float max_fps;
    uint32_t pixel_format;
};

// フレームコールバック
// pixel_format: VIDEO_PIXEL_FORMAT_NV12 / VIDEO_PIXEL_FORMAT_YUY2 / VIDEO_PIXEL_FORMAT_I420
// NV12 の場合: data は Y プレーン、uv_data は UV インターリーブプレーン
// YUY2 の場合: data はパックドデータ、uv_data は NULL
// I420 の場合: data は Y プレーン、uv_data は U プレーン + V プレーンを連結したデータ
// pixel_buffer: macOS の CVPixelBuffer (retained)。その他のプラットフォームでは必ず NULL（非 NULL は未サポート）
//
// コールバックはこの FFI 境界を跨いでアンワインド（パニック）してはならない。
// data および uv_data が指すバッファは、コールバックが返るまで有効である。非同期にスライスだけを保持して後から読んではならない。
// フレームをコールバック後も使う必要がある場合はコピーするか、上位 API の to_owned() を使うこと。
typedef void (*FrameCallback)(void* user_data,
                               const uint8_t* data,
                               const uint8_t* uv_data,
                               int width,
                               int height,
                               int stride,
                               int stride_uv,
                               uint32_t pixel_format,
                               int64_t timestamp_us,
                               void* pixel_buffer);

// デバイス列挙
// 成功時は 0、失敗時は負の値を返す
int video_enumerate_devices(struct VideoDevice*** devices, int* count);

// デバイス配列を解放
void video_free_devices(struct VideoDevice** devices, int count);

// デバイス名を取得（NULL 終端文字列）
const char* video_device_name(struct VideoDevice* device);

// デバイスの一意識別子を取得（NULL 終端文字列）
const char* video_device_unique_id(struct VideoDevice* device);

// デバイスの対応フォーマット数を取得
int video_device_format_count(struct VideoDevice* device);

// デバイスの対応フォーマットを取得（index は 0 から format_count - 1）
const struct VideoFormatEntry* video_device_get_format(struct VideoDevice* device, int index);

// セッションを作成
// device_id が NULL の場合はデフォルトデバイスを使用
// pixel_format が 0 の場合はデフォルト選択
struct VideoSession* video_session_create(const char* device_id,
                                           int width,
                                           int height,
                                           int fps,
                                           uint32_t pixel_format);

// セッションを破棄
void video_session_destroy(struct VideoSession* session);

// キャプチャを開始
// 成功時は 0、失敗時は負の値を返す
int video_session_start(struct VideoSession* session,
                         FrameCallback callback,
                         void* user_data);

// キャプチャを停止
void video_session_stop(struct VideoSession* session);

#if defined(__cplusplus)
}
#endif
