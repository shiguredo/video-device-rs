#include "video_v4l2.h"

#include <errno.h>
#include <fcntl.h>
#include <linux/videodev2.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <unistd.h>

#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG
// MJPEG ペイロード長の絶対上限 (256 MiB)
// 根拠: UVC 1.5 仕様のアイソクロナス転送最大ペイロードは High Speed (480 Mbps) で 3072 バイト/マイクロフレーム、
// SuperSpeed (5 Gbps) で 1024 バイト/マイクロフレーム。
// 実フレームサイズは 4K MJPEG で 5〜15 MiB、8K MJPEG で 20〜60 MiB、8K HDR で 90 MiB 超に達しうる。
// V4L2 の bytesused は u32 で最大 4 GiB だが、256 MiB は以下の理由で選択:
// 1. 将来の 8K や 16K 高フレームレートカメラを見越した十分な余裕値
// 2. int (32-bit signed) でサイズを扱う既存コードパスとの互換性 (256 MiB < INT32_MAX)
// 3. malloc/stack 割り当てにおける現実的な上限として過度に大きくない
// 異常ドライバや V4L2_BUF_FLAG_ERROR 付き巨大値に対する防御線として機能する。
static const size_t MJPEG_MAX_PAYLOAD_BYTES = 256u * 1024u * 1024u;
#endif

// VideoDevice 構造体
struct VideoDevice {
    char* name;
    char* unique_id;
    struct VideoFormatEntry* formats;
    int format_count;
};

// バッファ構造体
typedef struct {
    void* start;
    size_t length;
} Buffer;

// VideoSession 構造体
struct VideoSession {
    int fd;
    Buffer* buffers;
    unsigned int buffer_count;
    int width;
    int height;
    uint32_t pixel_format;
    FrameCallback callback;
    void* user_data;
    pthread_t thread;
    atomic_int running;
};

static int xioctl(int fd, unsigned long request, void* arg) {
    int r;
    do {
        r = ioctl(fd, request, arg);
    } while (r == -1 && errno == EINTR);
    return r;
}

// V4L2 ピクセルフォーマットを VIDEO_PIXEL_FORMAT_* に変換
static uint32_t convert_v4l2_pixel_format(uint32_t v4l2_format) {
    switch (v4l2_format) {
        case V4L2_PIX_FMT_NV12:
            return VIDEO_PIXEL_FORMAT_NV12;
        case V4L2_PIX_FMT_YUYV:
            return VIDEO_PIXEL_FORMAT_YUY2;
        case V4L2_PIX_FMT_YUV420:
            return VIDEO_PIXEL_FORMAT_I420;
#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG
        case V4L2_PIX_FMT_MJPEG:
            return VIDEO_PIXEL_FORMAT_MJPG;
#endif
        default:
            return 0;
    }
}

static uint32_t convert_video_pixel_format_to_v4l2(uint32_t pixel_format) {
    switch (pixel_format) {
        case VIDEO_PIXEL_FORMAT_NV12:
            return V4L2_PIX_FMT_NV12;
        case VIDEO_PIXEL_FORMAT_YUY2:
            return V4L2_PIX_FMT_YUYV;
        case VIDEO_PIXEL_FORMAT_I420:
            return V4L2_PIX_FMT_YUV420;
#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG
        case VIDEO_PIXEL_FORMAT_MJPG:
            return V4L2_PIX_FMT_MJPEG;
#endif
        default:
            return 0;
    }
}

// デバイスのフォーマット情報を列挙
static int enumerate_device_formats(const char* path, struct VideoFormatEntry** out_formats,
                                     int* out_count) {
    *out_formats = NULL;
    *out_count = 0;

    int fd = open(path, O_RDWR | O_NONBLOCK);
    if (fd < 0) {
        return -1;
    }

    // 一時的なフォーマット配列
    struct VideoFormatEntry* formats = NULL;
    int format_count = 0;
    int format_capacity = 0;

    // ピクセルフォーマットを列挙
    struct v4l2_fmtdesc fmtdesc;
    memset(&fmtdesc, 0, sizeof(fmtdesc));
    fmtdesc.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;

    while (xioctl(fd, VIDIOC_ENUM_FMT, &fmtdesc) == 0) {
        uint32_t converted = convert_v4l2_pixel_format(fmtdesc.pixelformat);
        if (converted == 0) {
            fmtdesc.index++;
            continue;
        }

        // フレームサイズを列挙
        struct v4l2_frmsizeenum frmsize;
        memset(&frmsize, 0, sizeof(frmsize));
        frmsize.pixel_format = fmtdesc.pixelformat;

        while (xioctl(fd, VIDIOC_ENUM_FRAMESIZES, &frmsize) == 0) {
            int width, height;

            if (frmsize.type == V4L2_FRMSIZE_TYPE_DISCRETE) {
                width = frmsize.discrete.width;
                height = frmsize.discrete.height;
            } else {
                // stepwise/continuous の場合は最大サイズを使用
                width = frmsize.stepwise.max_width;
                height = frmsize.stepwise.max_height;
            }

            // フレームレートを取得
            float min_fps = 1.0f;
            float max_fps = 30.0f;

            struct v4l2_frmivalenum frmival;
            memset(&frmival, 0, sizeof(frmival));
            frmival.pixel_format = fmtdesc.pixelformat;
            frmival.width = width;
            frmival.height = height;

            if (xioctl(fd, VIDIOC_ENUM_FRAMEINTERVALS, &frmival) == 0) {
                if (frmival.type == V4L2_FRMIVAL_TYPE_DISCRETE) {
                    // 最初のインターバルから fps を計算
                    if (frmival.discrete.numerator > 0) {
                        float fps = (float)frmival.discrete.denominator /
                                    (float)frmival.discrete.numerator;
                        min_fps = fps;
                        max_fps = fps;
                    }

                    // 全インターバルを走査して最小/最大を取得
                    while (xioctl(fd, VIDIOC_ENUM_FRAMEINTERVALS, &frmival) == 0) {
                        if (frmival.discrete.numerator > 0) {
                            float fps = (float)frmival.discrete.denominator /
                                        (float)frmival.discrete.numerator;
                            if (fps < min_fps)
                                min_fps = fps;
                            if (fps > max_fps)
                                max_fps = fps;
                        }
                        frmival.index++;
                    }
                } else if (frmival.type == V4L2_FRMIVAL_TYPE_STEPWISE ||
                           frmival.type == V4L2_FRMIVAL_TYPE_CONTINUOUS) {
                    if (frmival.stepwise.max.numerator > 0) {
                        min_fps = (float)frmival.stepwise.max.denominator /
                                  (float)frmival.stepwise.max.numerator;
                    }
                    if (frmival.stepwise.min.numerator > 0) {
                        max_fps = (float)frmival.stepwise.min.denominator /
                                  (float)frmival.stepwise.min.numerator;
                    }
                }
            }

            // 配列を拡張
            if (format_count >= format_capacity) {
                format_capacity = format_capacity == 0 ? 32 : format_capacity * 2;
                struct VideoFormatEntry* new_formats =
                    (struct VideoFormatEntry*)realloc(formats, sizeof(struct VideoFormatEntry) * format_capacity);
                if (!new_formats) {
                    free(formats);
                    close(fd);
                    return -1;
                }
                formats = new_formats;
            }

            formats[format_count].width = width;
            formats[format_count].height = height;
            formats[format_count].min_fps = min_fps;
            formats[format_count].max_fps = max_fps;
            formats[format_count].pixel_format = converted;
            format_count++;

            // stepwise/continuous の場合は 1 エントリのみ
            if (frmsize.type != V4L2_FRMSIZE_TYPE_DISCRETE) {
                break;
            }

            frmsize.index++;
        }

        fmtdesc.index++;
    }

    close(fd);

    *out_formats = formats;
    *out_count = format_count;
    return 0;
}

int video_v4l2_enumerate_devices(struct VideoDevice*** devices, int* count) {
    if (!devices || !count) {
        return -1;
    }

    // /dev/video* デバイスを列挙
    struct VideoDevice** device_array = NULL;
    int device_count = 0;
    int capacity = 0;

    for (int i = 0; i < 64; i++) {
        char path[32];
        snprintf(path, sizeof(path), "/dev/video%d", i);

        struct stat st;
        if (stat(path, &st) != 0) {
            continue;
        }

        if (!S_ISCHR(st.st_mode)) {
            continue;
        }

        int fd = open(path, O_RDWR | O_NONBLOCK);
        if (fd < 0) {
            continue;
        }

        struct v4l2_capability cap;
        if (xioctl(fd, VIDIOC_QUERYCAP, &cap) < 0) {
            close(fd);
            continue;
        }

        // ビデオキャプチャデバイスかどうか確認
        if (!(cap.device_caps & V4L2_CAP_VIDEO_CAPTURE)) {
            close(fd);
            continue;
        }

        // ストリーミングをサポートしているか確認
        if (!(cap.device_caps & V4L2_CAP_STREAMING)) {
            close(fd);
            continue;
        }

        close(fd);

        // 配列を拡張
        if (device_count >= capacity) {
            capacity = capacity == 0 ? 8 : capacity * 2;
            struct VideoDevice** new_array =
                (struct VideoDevice**)realloc(device_array, sizeof(struct VideoDevice*) * capacity);
            if (!new_array) {
                // 既存のデバイスを解放
                for (int j = 0; j < device_count; j++) {
                    free(device_array[j]->name);
                    free(device_array[j]->unique_id);
                    free(device_array[j]);
                }
                free(device_array);
                return -2;
            }
            device_array = new_array;
        }

        struct VideoDevice* device =
            (struct VideoDevice*)malloc(sizeof(struct VideoDevice));
        if (!device) {
            for (int j = 0; j < device_count; j++) {
                free(device_array[j]->name);
                free(device_array[j]->unique_id);
                free(device_array[j]);
            }
            free(device_array);
            return -2;
        }

        device->name = strdup((const char*)cap.card);
        device->unique_id = strdup(path);
        device->formats = NULL;
        device->format_count = 0;

        // フォーマット情報を取得
        enumerate_device_formats(path, &device->formats, &device->format_count);

        device_array[device_count++] = device;
    }

    *devices = device_array;
    *count = device_count;
    return 0;
}

void video_v4l2_free_devices(struct VideoDevice** devices, int count) {
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

const char* video_v4l2_device_name(struct VideoDevice* device) {
    if (!device) {
        return NULL;
    }
    return device->name;
}

const char* video_v4l2_device_unique_id(struct VideoDevice* device) {
    if (!device) {
        return NULL;
    }
    return device->unique_id;
}

int video_v4l2_device_format_count(struct VideoDevice* device) {
    if (!device) {
        return 0;
    }
    return device->format_count;
}

const struct VideoFormatEntry* video_v4l2_device_get_format(struct VideoDevice* device, int index) {
    if (!device || index < 0 || index >= device->format_count) {
        return NULL;
    }
    return &device->formats[index];
}

static void cleanup_mmap(struct VideoSession* session) {
    if (session->buffers) {
        for (unsigned int i = 0; i < session->buffer_count; i++) {
            if (session->buffers[i].start && session->buffers[i].start != MAP_FAILED) {
                munmap(session->buffers[i].start, session->buffers[i].length);
            }
        }
        free(session->buffers);
        session->buffers = NULL;
        session->buffer_count = 0;
    }
}

static int init_mmap(struct VideoSession* session) {
    struct v4l2_requestbuffers req;
    memset(&req, 0, sizeof(req));
    req.count = 4;
    req.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    req.memory = V4L2_MEMORY_MMAP;

    if (xioctl(session->fd, VIDIOC_REQBUFS, &req) < 0) {
        return -1;
    }

    if (req.count < 2) {
        return -1;
    }

    session->buffers = (Buffer*)calloc(req.count, sizeof(Buffer));
    if (!session->buffers) {
        return -1;
    }

    session->buffer_count = req.count;

    for (unsigned int i = 0; i < req.count; i++) {
        struct v4l2_buffer buf;
        memset(&buf, 0, sizeof(buf));
        buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        buf.memory = V4L2_MEMORY_MMAP;
        buf.index = i;

        if (xioctl(session->fd, VIDIOC_QUERYBUF, &buf) < 0) {
            goto fail;
        }

        session->buffers[i].length = buf.length;
        session->buffers[i].start =
            mmap(NULL, buf.length, PROT_READ | PROT_WRITE, MAP_SHARED, session->fd, buf.m.offset);

        if (session->buffers[i].start == MAP_FAILED) {
            goto fail;
        }
    }

    return 0;

fail:
    // 部分失敗時: 既に mmap した領域と buffers 配列を解放する
    cleanup_mmap(session);
    return -1;
}

struct VideoSession* video_v4l2_session_create(const char* device_id, int width,
                                          int height, int fps,
                                          uint32_t requested_pixel_format) {
    const char* device_path = device_id ? device_id : "/dev/video0";

    int fd = open(device_path, O_RDWR | O_NONBLOCK);
    if (fd < 0) {
        return NULL;
    }

    struct v4l2_capability cap;
    if (xioctl(fd, VIDIOC_QUERYCAP, &cap) < 0) {
        close(fd);
        return NULL;
    }

    if (!(cap.device_caps & V4L2_CAP_VIDEO_CAPTURE) ||
        !(cap.device_caps & V4L2_CAP_STREAMING)) {
        close(fd);
        return NULL;
    }

    // フォーマットを設定
    struct v4l2_format fmt;
    memset(&fmt, 0, sizeof(fmt));
    fmt.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    fmt.fmt.pix.width = width;
    fmt.fmt.pix.height = height;
    fmt.fmt.pix.field = V4L2_FIELD_NONE;

    uint32_t pixel_format = 0;
    if (requested_pixel_format != 0) {
        pixel_format = convert_video_pixel_format_to_v4l2(requested_pixel_format);
        if (pixel_format == 0) {
            close(fd);
            return NULL;
        }
        fmt.fmt.pix.pixelformat = pixel_format;
        if (xioctl(fd, VIDIOC_S_FMT, &fmt) < 0 || fmt.fmt.pix.pixelformat != pixel_format) {
            close(fd);
            return NULL;
        }
    } else {
        // NV12 を優先的に試す
        pixel_format = V4L2_PIX_FMT_NV12;
        fmt.fmt.pix.pixelformat = V4L2_PIX_FMT_NV12;

        if (xioctl(fd, VIDIOC_S_FMT, &fmt) < 0 ||
            fmt.fmt.pix.pixelformat != V4L2_PIX_FMT_NV12) {
            // YUY2 を試す
            fmt.fmt.pix.pixelformat = V4L2_PIX_FMT_YUYV;
            if (xioctl(fd, VIDIOC_S_FMT, &fmt) < 0 ||
                fmt.fmt.pix.pixelformat != V4L2_PIX_FMT_YUYV) {
                close(fd);
                return NULL;
            }
            pixel_format = V4L2_PIX_FMT_YUYV;
        }
    }

    // フレームレートを設定
    struct v4l2_streamparm parm;
    memset(&parm, 0, sizeof(parm));
    parm.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    parm.parm.capture.timeperframe.numerator = 1;
    parm.parm.capture.timeperframe.denominator = fps;
    xioctl(fd, VIDIOC_S_PARM, &parm);

    struct VideoSession* session =
        (struct VideoSession*)calloc(1, sizeof(struct VideoSession));
    if (!session) {
        close(fd);
        return NULL;
    }

    session->fd = fd;
    session->width = fmt.fmt.pix.width;
    session->height = fmt.fmt.pix.height;
    session->pixel_format = pixel_format;
    atomic_init(&session->running, 0);

    if (init_mmap(session) < 0) {
        close(fd);
        free(session);
        return NULL;
    }

    return session;
}

void video_v4l2_session_destroy(struct VideoSession* session) {
    if (!session) {
        return;
    }

    if (atomic_load(&session->running)) {
        video_v4l2_session_stop(session);
    }

    cleanup_mmap(session);

    if (session->fd >= 0) {
        close(session->fd);
    }

    free(session);
}

static void* capture_thread(void* arg) {
    struct VideoSession* session = (struct VideoSession*)arg;

    while (atomic_load(&session->running)) {
        fd_set fds;
        FD_ZERO(&fds);
        FD_SET(session->fd, &fds);

        struct timeval tv;
        tv.tv_sec = 1;
        tv.tv_usec = 0;

        int r = select(session->fd + 1, &fds, NULL, NULL, &tv);
        if (r < 0) {
            if (errno == EINTR) {
                continue;
            }
            break;
        }

        if (r == 0) {
            // タイムアウト
            continue;
        }

        struct v4l2_buffer buf;
        memset(&buf, 0, sizeof(buf));
        buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        buf.memory = V4L2_MEMORY_MMAP;

        if (xioctl(session->fd, VIDIOC_DQBUF, &buf) < 0) {
            if (errno == EAGAIN) {
                continue;
            }
            break;
        }

        // タイムスタンプをマイクロ秒に変換
        int64_t timestamp_us =
            (int64_t)buf.timestamp.tv_sec * 1000000 + (int64_t)buf.timestamp.tv_usec;

        // ドライバが不正な index を返した場合の防御
        if (buf.index >= session->buffer_count) {
            break;
        }

        if (session->callback) {
            const uint8_t* data = (const uint8_t*)session->buffers[buf.index].start;
            size_t mmap_len = session->buffers[buf.index].length;
            // bytesused が 0 の場合はドライバ不具合とみなしてスキップ
            if (buf.bytesused == 0) {
                goto requeue;
            }
            uint32_t used = buf.bytesused;
            // 有効データ長は mmap 長と bytesused の小さい方で制限する
            size_t available = (size_t)used < mmap_len ? (size_t)used : mmap_len;

            if (session->pixel_format == V4L2_PIX_FMT_NV12) {
                // NV12: Y プレーンと UV プレーンが連続
                size_t y_size = (size_t)session->width * (size_t)session->height;
                // UV 行数は height/2 切り上げ
                size_t uv_h = ((size_t)session->height + 1u) / 2u;
                size_t uv_size = (size_t)session->width * uv_h;
                size_t need = y_size + uv_size;
                if (need > available) {
                    goto requeue;
                }
                const uint8_t* uv_data = data + y_size;

                session->callback(session->user_data, data, uv_data, session->width,
                                  session->height, session->width, session->width,
                                  VIDEO_PIXEL_FORMAT_NV12, timestamp_us, NULL);
            } else if (session->pixel_format == V4L2_PIX_FMT_YUV420) {
                // YUV420: Y, U, V プレーンが連続
                size_t y_size = (size_t)session->width * (size_t)session->height;
                size_t chroma_h = ((size_t)session->height + 1u) / 2u;
                int stride_uv = (session->width + 1) / 2;
                size_t uv_total = (size_t)stride_uv * chroma_h * 2u;
                size_t need = y_size + uv_total;
                if (need > available) {
                    goto requeue;
                }
                const uint8_t* uv_data = data + y_size;
                session->callback(session->user_data, data, uv_data, session->width,
                                session->height, session->width, stride_uv,
                                VIDEO_PIXEL_FORMAT_I420, timestamp_us, NULL);
            } else if (session->pixel_format == V4L2_PIX_FMT_YUYV) {
                // YUY2: パックドフォーマット
                size_t need = (size_t)session->width * 2 * (size_t)session->height;
                if (need > available) {
                    goto requeue;
                }
                session->callback(session->user_data, data, NULL, session->width, session->height,
                                  session->width * 2, 0, VIDEO_PIXEL_FORMAT_YUY2, timestamp_us,
                                  NULL);
#ifdef SHIGUREDO_VIDEO_DEVICE_MJPEG
            } else if (session->pixel_format == V4L2_PIX_FMT_MJPEG) {
                if (available > MJPEG_MAX_PAYLOAD_BYTES) {
                    goto requeue;
                }
                session->callback(session->user_data, data, NULL,
                                  session->width, session->height,
                                  (int)available, 0,
                                  VIDEO_PIXEL_FORMAT_MJPG, timestamp_us, NULL);
#endif
            }
        }

requeue:
        // バッファをキューに戻す
        if (xioctl(session->fd, VIDIOC_QBUF, &buf) < 0) {
            break;
        }
    }

    return NULL;
}

int video_v4l2_session_start(struct VideoSession* session, FrameCallback callback, void* user_data) {
    if (!session || !callback) {
        return -1;
    }

    if (atomic_load(&session->running)) {
        return 0;
    }

    session->callback = callback;
    session->user_data = user_data;

    // バッファをキューに追加
    for (unsigned int i = 0; i < session->buffer_count; i++) {
        struct v4l2_buffer buf;
        memset(&buf, 0, sizeof(buf));
        buf.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        buf.memory = V4L2_MEMORY_MMAP;
        buf.index = i;

        if (xioctl(session->fd, VIDIOC_QBUF, &buf) < 0) {
            return -1;
        }
    }

    // ストリーミングを開始
    enum v4l2_buf_type type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    if (xioctl(session->fd, VIDIOC_STREAMON, &type) < 0) {
        return -1;
    }

    atomic_store(&session->running, 1);

    // キャプチャスレッドを開始
    if (pthread_create(&session->thread, NULL, capture_thread, session) != 0) {
        atomic_store(&session->running, 0);
        xioctl(session->fd, VIDIOC_STREAMOFF, &type);
        return -1;
    }

    return 0;
}

void video_v4l2_session_stop(struct VideoSession* session) {
    if (!session || !atomic_load(&session->running)) {
        return;
    }

    atomic_store(&session->running, 0);

    pthread_join(session->thread, NULL);

    enum v4l2_buf_type type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    xioctl(session->fd, VIDIOC_STREAMOFF, &type);
}
