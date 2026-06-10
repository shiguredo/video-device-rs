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
// MJPEG は圧縮フォーマット。フレームは可変長の JPEG ペイロード。
#define VIDEO_PIXEL_FORMAT_MJPG 0x47504A4D  // 'MJPG' (Motion JPEG, V4L2 互換)

// フォーマットエントリ
struct VideoFormatEntry {
    int width;
    int height;
    float min_fps;
    float max_fps;
    uint32_t pixel_format;
};

// フレームコールバック
// pixel_format: VIDEO_PIXEL_FORMAT_NV12 / VIDEO_PIXEL_FORMAT_YUY2 / VIDEO_PIXEL_FORMAT_I420 / VIDEO_PIXEL_FORMAT_MJPG
// NV12 の場合: data は Y プレーン、uv_data は UV インターリーブプレーン
// YUY2 の場合: data はパックドデータ、uv_data は NULL
// I420 の場合: data は Y プレーン、uv_data は U プレーン + V プレーンを連結したデータ
// MJPEG の場合: data は JPEG ペイロード先頭、uv_data は NULL、stride 引数は JPEG ペイロード長 (バイト) (バイト/行ではない)、
//   stride_uv は 0。pixel_format によって stride の単位が異なる契約であることに注意。
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

#if defined(__cplusplus)
}
#endif
