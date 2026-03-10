#include "video_c.h"

#include <errno.h>
#include <fcntl.h>
#include <linux/videodev2.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <unistd.h>

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
    volatile int running;
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

int video_enumerate_devices(struct VideoDevice*** devices, int* count) {
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
            return -1;
        }

        session->buffers[i].length = buf.length;
        session->buffers[i].start =
            mmap(NULL, buf.length, PROT_READ | PROT_WRITE, MAP_SHARED, session->fd, buf.m.offset);

        if (session->buffers[i].start == MAP_FAILED) {
            return -1;
        }
    }

    return 0;
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
    }
}

struct VideoSession* video_session_create(const char* device_id, int width,
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
    session->running = 0;

    if (init_mmap(session) < 0) {
        close(fd);
        free(session);
        return NULL;
    }

    return session;
}

void video_session_destroy(struct VideoSession* session) {
    if (!session) {
        return;
    }

    if (session->running) {
        video_session_stop(session);
    }

    cleanup_mmap(session);

    if (session->fd >= 0) {
        close(session->fd);
    }

    free(session);
}

static void* capture_thread(void* arg) {
    struct VideoSession* session = (struct VideoSession*)arg;

    while (session->running) {
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

        if (session->callback) {
            const uint8_t* data = (const uint8_t*)session->buffers[buf.index].start;

            if (session->pixel_format == V4L2_PIX_FMT_NV12) {
                // NV12: Y プレーンと UV プレーンが連続
                int y_size = session->width * session->height;
                const uint8_t* uv_data = data + y_size;

                session->callback(session->user_data, data, uv_data, session->width,
                                  session->height, session->width, session->width,
                                  VIDEO_PIXEL_FORMAT_NV12, timestamp_us);
            } else if (session->pixel_format == V4L2_PIX_FMT_YUYV) {
                // YUY2: パックドフォーマット
                session->callback(session->user_data, data, NULL, session->width, session->height,
                                  session->width * 2, 0, VIDEO_PIXEL_FORMAT_YUY2, timestamp_us);
            }
        }

        // バッファをキューに戻す
        if (xioctl(session->fd, VIDIOC_QBUF, &buf) < 0) {
            break;
        }
    }

    return NULL;
}

int video_session_start(struct VideoSession* session, FrameCallback callback, void* user_data) {
    if (!session || !callback) {
        return -1;
    }

    if (session->running) {
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

    session->running = 1;

    // キャプチャスレッドを開始
    if (pthread_create(&session->thread, NULL, capture_thread, session) != 0) {
        session->running = 0;
        xioctl(session->fd, VIDIOC_STREAMOFF, &type);
        return -1;
    }

    return 0;
}

void video_session_stop(struct VideoSession* session) {
    if (!session || !session->running) {
        return;
    }

    session->running = 0;

    pthread_join(session->thread, NULL);

    enum v4l2_buf_type type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
    xioctl(session->fd, VIDIOC_STREAMOFF, &type);
}
