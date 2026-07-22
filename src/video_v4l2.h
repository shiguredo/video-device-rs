#pragma once

#include "video.h"

#if defined(__cplusplus)
extern "C" {
#endif

// デバイス列挙
// 成功時は 0、失敗時は負の値を返す
int video_v4l2_enumerate_devices(struct VideoDevice*** devices, int* count);

// デバイス配列を解放
void video_v4l2_free_devices(struct VideoDevice** devices, int count);

// デバイス名を取得（NULL 終端文字列）
const char* video_v4l2_device_name(struct VideoDevice* device);

// デバイスの一意識別子を取得（NULL 終端文字列）
const char* video_v4l2_device_unique_id(struct VideoDevice* device);

// デバイスの対応フォーマット数を取得
int video_v4l2_device_format_count(struct VideoDevice* device);

// デバイスの対応フォーマットを取得（index は 0 から format_count - 1）
const struct VideoFormatEntry* video_v4l2_device_get_format(struct VideoDevice* device, int index);

// セッションを作成
// device_id が NULL の場合はデフォルトデバイスを使用
// pixel_format が 0 の場合はデフォルト選択
struct VideoSession* video_v4l2_session_create(const char* device_id,
                                                int width,
                                                int height,
                                                int fps,
                                                uint32_t pixel_format);

// セッションを破棄
void video_v4l2_session_destroy(struct VideoSession* session);

// キャプチャを開始
// 成功時は 0、失敗時は負の値を返す
int video_v4l2_session_start(struct VideoSession* session,
                              FrameCallback callback,
                              void* user_data);

// キャプチャを停止
void video_v4l2_session_stop(struct VideoSession* session);

#if defined(__cplusplus)
}
#endif
