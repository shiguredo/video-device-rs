#import <AVFoundation/AVFoundation.h>
#import <CoreFoundation/CoreFoundation.h>
#import <CoreMedia/CoreMedia.h>
#import <CoreVideo/CoreVideo.h>
#import <Foundation/Foundation.h>

#include "video_c.h"

// VideoDevice 構造体
struct VideoDevice {
    char* name;
    char* unique_id;
    struct VideoFormatEntry* formats;
    int format_count;
};

// VideoCaptureDelegate: フレームを受け取るデリゲート
@interface VideoCaptureDelegate : NSObject <AVCaptureVideoDataOutputSampleBufferDelegate>
@property (nonatomic, assign) FrameCallback callback;
@property (nonatomic, assign) void* userData;
@end

@implementation VideoCaptureDelegate

- (void)captureOutput:(AVCaptureOutput*)output
    didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
           fromConnection:(AVCaptureConnection*)connection {
    if (!self.callback) {
        return;
    }

    CVImageBufferRef imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer);
    if (!imageBuffer) {
        return;
    }

    CVPixelBufferLockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);

    OSType pixelFormat = CVPixelBufferGetPixelFormatType(imageBuffer);
    size_t width = CVPixelBufferGetWidth(imageBuffer);
    size_t height = CVPixelBufferGetHeight(imageBuffer);

    // タイムスタンプを取得（マイクロ秒）
    CMTime pts = CMSampleBufferGetPresentationTimeStamp(sampleBuffer);
    int64_t timestamp_us = (int64_t)(CMTimeGetSeconds(pts) * 1000000.0);

    if (pixelFormat == kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange ||
        pixelFormat == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange) {
        // NV12 形式
        const uint8_t* yPlane = CVPixelBufferGetBaseAddressOfPlane(imageBuffer, 0);
        const uint8_t* uvPlane = CVPixelBufferGetBaseAddressOfPlane(imageBuffer, 1);
        size_t strideY = CVPixelBufferGetBytesPerRowOfPlane(imageBuffer, 0);
        size_t strideUV = CVPixelBufferGetBytesPerRowOfPlane(imageBuffer, 1);

        CFRetain(imageBuffer);
        self.callback(self.userData, yPlane, uvPlane, (int)width, (int)height,
                      (int)strideY, (int)strideUV, VIDEO_PIXEL_FORMAT_NV12,
                      timestamp_us, (void*)imageBuffer);
    } else if (pixelFormat == kCVPixelFormatType_422YpCbCr8_yuvs ||
               pixelFormat == kCVPixelFormatType_422YpCbCr8) {
        // YUY2 形式
        const uint8_t* data = CVPixelBufferGetBaseAddress(imageBuffer);
        size_t stride = CVPixelBufferGetBytesPerRow(imageBuffer);

        CFRetain(imageBuffer);
        self.callback(self.userData, data, NULL, (int)width, (int)height,
                      (int)stride, 0, VIDEO_PIXEL_FORMAT_YUY2, timestamp_us,
                      (void*)imageBuffer);
    } else if (pixelFormat == kCVPixelFormatType_420YpCbCr8Planar ||
               pixelFormat == kCVPixelFormatType_420YpCbCr8PlanarFullRange) {
        // I420 形式。U/V 平面は連結バッファへ詰め替える
        const uint8_t* yPlane = CVPixelBufferGetBaseAddressOfPlane(imageBuffer, 0);
        const uint8_t* uPlane = CVPixelBufferGetBaseAddressOfPlane(imageBuffer, 1);
        const uint8_t* vPlane = CVPixelBufferGetBaseAddressOfPlane(imageBuffer, 2);
        size_t strideY = CVPixelBufferGetBytesPerRowOfPlane(imageBuffer, 0);
        size_t strideU = CVPixelBufferGetBytesPerRowOfPlane(imageBuffer, 1);
        size_t strideV = CVPixelBufferGetBytesPerRowOfPlane(imageBuffer, 2);
        // chromaHeight は U/V の行数。Rust 側 `i420_plane_sizes` の UV バイト数は
        // `(stride_uv as usize) * ((height + 1) / 2) * 2` であり、本 uvSize と一致する。
        size_t chromaHeight = (height + 1) / 2;
        size_t strideUV = strideU > strideV ? strideU : strideV;
        size_t uvSize = strideUV * chromaHeight * 2;
        uint8_t* uvBuffer = (uint8_t*)calloc(1, uvSize);

        if (uvBuffer) {
            for (size_t row = 0; row < chromaHeight; row++) {
                memcpy(uvBuffer + row * strideUV, uPlane + row * strideU, strideU);
                memcpy(uvBuffer + strideUV * chromaHeight + row * strideUV,
                       vPlane + row * strideV, strideV);
            }

            CFRetain(imageBuffer);
            self.callback(self.userData, yPlane, uvBuffer, (int)width, (int)height,
                          (int)strideY, (int)strideUV, VIDEO_PIXEL_FORMAT_I420,
                          timestamp_us, (void*)imageBuffer);
            free(uvBuffer);
        }
    }

    CVPixelBufferUnlockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
}

@end

// VideoSession 構造体
struct VideoSession {
    AVCaptureSession* session;
    AVCaptureDeviceInput* input;
    AVCaptureVideoDataOutput* output;
    VideoCaptureDelegate* delegate;
    dispatch_queue_t queue;
};

// ピクセルフォーマットを VIDEO_PIXEL_FORMAT_* に変換
static uint32_t convert_pixel_format(OSType format) {
    switch (format) {
        case kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange:
        case kCVPixelFormatType_420YpCbCr8BiPlanarFullRange:
            return VIDEO_PIXEL_FORMAT_NV12;
        case kCVPixelFormatType_422YpCbCr8_yuvs:
        case kCVPixelFormatType_422YpCbCr8:
            return VIDEO_PIXEL_FORMAT_YUY2;
        case kCVPixelFormatType_420YpCbCr8Planar:
        case kCVPixelFormatType_420YpCbCr8PlanarFullRange:
            return VIDEO_PIXEL_FORMAT_I420;
        default:
            return 0;
    }
}

static OSType convert_video_pixel_format_to_cv(uint32_t pixel_format) {
    switch (pixel_format) {
        case 0:
        case VIDEO_PIXEL_FORMAT_NV12:
            return kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange;
        case VIDEO_PIXEL_FORMAT_YUY2:
            return kCVPixelFormatType_422YpCbCr8_yuvs;
        case VIDEO_PIXEL_FORMAT_I420:
            return kCVPixelFormatType_420YpCbCr8Planar;
        default:
            return 0;
    }
}

static BOOL output_supports_pixel_format(AVCaptureVideoDataOutput* output,
                                         OSType pixelFormat) {
    for (NSNumber* value in output.availableVideoCVPixelFormatTypes) {
        if ((OSType)value.unsignedIntValue == pixelFormat) {
            return YES;
        }
    }
    return NO;
}

int video_enumerate_devices(struct VideoDevice*** devices, int* count) {
    if (!devices || !count) {
        return -1;
    }

    AVCaptureDeviceDiscoverySession* discoverySession = [AVCaptureDeviceDiscoverySession
        discoverySessionWithDeviceTypes:@[AVCaptureDeviceTypeBuiltInWideAngleCamera,
                                           AVCaptureDeviceTypeExternal]
                              mediaType:AVMediaTypeVideo
                               position:AVCaptureDevicePositionUnspecified];

    NSArray<AVCaptureDevice*>* captureDevices = discoverySession.devices;
    NSUInteger deviceCount = captureDevices.count;

    if (deviceCount == 0) {
        *devices = NULL;
        *count = 0;
        return 0;
    }

    struct VideoDevice** deviceArray =
        (struct VideoDevice**)malloc(sizeof(struct VideoDevice*) * deviceCount);
    if (!deviceArray) {
        return -2;
    }

    for (NSUInteger i = 0; i < deviceCount; i++) {
        AVCaptureDevice* device = captureDevices[i];
        struct VideoDevice* videoDevice =
            (struct VideoDevice*)malloc(sizeof(struct VideoDevice));
        if (!videoDevice) {
            // 既に確保したメモリを解放
            for (NSUInteger j = 0; j < i; j++) {
                free(deviceArray[j]->name);
                free(deviceArray[j]->unique_id);
                free(deviceArray[j]->formats);
                free(deviceArray[j]);
            }
            free(deviceArray);
            return -2;
        }

        const char* name = [device.localizedName UTF8String];
        const char* uniqueId = [device.uniqueID UTF8String];

        videoDevice->name = strdup(name);
        videoDevice->unique_id = strdup(uniqueId);
        videoDevice->formats = NULL;
        videoDevice->format_count = 0;

        // フォーマット情報を収集
        NSArray<AVCaptureDeviceFormat*>* formats = device.formats;
        NSUInteger formatCount = formats.count;

        if (formatCount > 0) {
            // 一時的に最大サイズで確保（後で実際の数に調整）
            struct VideoFormatEntry* formatArray =
                (struct VideoFormatEntry*)malloc(sizeof(struct VideoFormatEntry) * formatCount);

            if (formatArray) {
                int actualCount = 0;

                for (AVCaptureDeviceFormat* format in formats) {
                    CMFormatDescriptionRef desc = format.formatDescription;
                    CMVideoDimensions dims = CMVideoFormatDescriptionGetDimensions(desc);
                    OSType pixelFormat =
                        CMFormatDescriptionGetMediaSubType(desc);

                    uint32_t convertedFormat = convert_pixel_format(pixelFormat);
                    if (convertedFormat == 0) {
                        // サポートされていないフォーマットはスキップ
                        continue;
                    }

                    // フレームレート範囲を取得
                    float minFps = 1.0f;
                    float maxFps = 30.0f;

                    NSArray<AVFrameRateRange*>* frameRateRanges =
                        format.videoSupportedFrameRateRanges;
                    if (frameRateRanges.count > 0) {
                        AVFrameRateRange* range = frameRateRanges[0];
                        minFps = (float)range.minFrameRate;
                        maxFps = (float)range.maxFrameRate;

                        // 複数の範囲がある場合は最小と最大を取得
                        for (AVFrameRateRange* r in frameRateRanges) {
                            if (r.minFrameRate < minFps) {
                                minFps = (float)r.minFrameRate;
                            }
                            if (r.maxFrameRate > maxFps) {
                                maxFps = (float)r.maxFrameRate;
                            }
                        }
                    }

                    formatArray[actualCount].width = dims.width;
                    formatArray[actualCount].height = dims.height;
                    formatArray[actualCount].min_fps = minFps;
                    formatArray[actualCount].max_fps = maxFps;
                    formatArray[actualCount].pixel_format = convertedFormat;
                    actualCount++;
                }

                if (actualCount > 0) {
                    videoDevice->formats = formatArray;
                    videoDevice->format_count = actualCount;
                } else {
                    free(formatArray);
                }
            }
        }

        deviceArray[i] = videoDevice;
    }

    *devices = deviceArray;
    *count = (int)deviceCount;
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

struct VideoSession* video_session_create(const char* device_id, int width,
                                          int height, int fps,
                                          uint32_t requested_pixel_format) {
    // カメラアクセス権限を確認する。未認可の場合はフレームが配信されないため
    // セッション作成前にエラーを返す
    AVAuthorizationStatus authStatus =
        [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeVideo];
    if (authStatus == AVAuthorizationStatusDenied ||
        authStatus == AVAuthorizationStatusRestricted) {
        return NULL;
    }
    if (authStatus == AVAuthorizationStatusNotDetermined) {
        // 権限が未決定の場合、同期的に要求する
        dispatch_semaphore_t sema = dispatch_semaphore_create(0);
        __block BOOL granted = NO;
        [AVCaptureDevice requestAccessForMediaType:AVMediaTypeVideo
                                 completionHandler:^(BOOL result) {
            granted = result;
            dispatch_semaphore_signal(sema);
        }];
        dispatch_semaphore_wait(sema, DISPATCH_TIME_FOREVER);
        if (!granted) {
            return NULL;
        }
    }

    AVCaptureDevice* device = nil;

    if (device_id) {
        NSString* deviceIdStr = [NSString stringWithUTF8String:device_id];
        device = [AVCaptureDevice deviceWithUniqueID:deviceIdStr];
    }

    if (!device) {
        // デフォルトのビデオデバイスを取得
        device = [AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeVideo];
    }

    if (!device) {
        return NULL;
    }

    struct VideoSession* videoSession =
        (struct VideoSession*)malloc(sizeof(struct VideoSession));
    if (!videoSession) {
        return NULL;
    }

    // セッションを作成
    AVCaptureSession* session = [[AVCaptureSession alloc] init];

    // 入力を作成
    NSError* error = nil;
    AVCaptureDeviceInput* input = [AVCaptureDeviceInput deviceInputWithDevice:device error:&error];
    if (!input || error) {
        free(videoSession);
        return NULL;
    }

    if (![session canAddInput:input]) {
        free(videoSession);
        return NULL;
    }
    [session addInput:input];

    // 出力を作成
    AVCaptureVideoDataOutput* output = [[AVCaptureVideoDataOutput alloc] init];
    output.alwaysDiscardsLateVideoFrames = YES;

    OSType outputPixelFormat =
        convert_video_pixel_format_to_cv(requested_pixel_format);
    if (outputPixelFormat == 0 ||
        !output_supports_pixel_format(output, outputPixelFormat)) {
        free(videoSession);
        return NULL;
    }

    // 出力形式を指定し、出力解像度を明示指定する
    // macOS では sessionPreset のデフォルト（AVCaptureSessionPresetHigh = 1080p）が
    // device.activeFormat の解像度を上書きするため、videoSettings で幅・高さを指定する必要がある
    output.videoSettings = @{
        (NSString*)kCVPixelBufferPixelFormatTypeKey:
            @(outputPixelFormat),
        (NSString*)kCVPixelBufferWidthKey: @(width),
        (NSString*)kCVPixelBufferHeightKey: @(height),
    };

    if (![session canAddOutput:output]) {
        free(videoSession);
        return NULL;
    }
    [session addOutput:output];

    // デバイスの解像度とフレームレートを設定
    if ([device lockForConfiguration:&error]) {
        // 解像度に最も近いフォーマットを選択（フレームレート設定より先に行う）
        AVCaptureDeviceFormat* bestFormat = nil;
        int bestDiff = INT_MAX;

        for (AVCaptureDeviceFormat* format in device.formats) {
            CMVideoDimensions dims = CMVideoFormatDescriptionGetDimensions(format.formatDescription);
            int diff = abs(dims.width - width) + abs(dims.height - height);
            if (diff < bestDiff) {
                bestDiff = diff;
                bestFormat = format;
            }
        }

        if (bestFormat) {
            device.activeFormat = bestFormat;
        }

        // フレームレートを設定（activeFormat の対応範囲内に収める）
        CMTime frameDuration = CMTimeMake(1, fps);
        AVCaptureDeviceFormat* activeFormat = device.activeFormat;
        BOOL fpsSupported = NO;

        for (AVFrameRateRange* range in activeFormat.videoSupportedFrameRateRanges) {
            if (CMTimeCompare(frameDuration, range.minFrameDuration) >= 0 &&
                CMTimeCompare(frameDuration, range.maxFrameDuration) <= 0) {
                fpsSupported = YES;
                break;
            }
        }

        if (fpsSupported) {
            device.activeVideoMinFrameDuration = frameDuration;
            device.activeVideoMaxFrameDuration = frameDuration;
        } else if (activeFormat.videoSupportedFrameRateRanges.count > 0) {
            // 指定 fps が対応範囲外の場合、最も近い対応フレームレートを使用する
            AVFrameRateRange* bestRange = activeFormat.videoSupportedFrameRateRanges[0];
            double targetFps = (double)fps;
            double bestDelta = fabs(bestRange.maxFrameRate - targetFps);

            for (AVFrameRateRange* range in activeFormat.videoSupportedFrameRateRanges) {
                double delta = fabs(range.maxFrameRate - targetFps);
                if (delta < bestDelta) {
                    bestDelta = delta;
                    bestRange = range;
                }
            }

            device.activeVideoMinFrameDuration = bestRange.minFrameDuration;
            device.activeVideoMaxFrameDuration = bestRange.maxFrameDuration;
        }

        [device unlockForConfiguration];
    }

    // デリゲートを作成
    VideoCaptureDelegate* delegate = [[VideoCaptureDelegate alloc] init];
    dispatch_queue_t queue = dispatch_queue_create("video.capture.queue", DISPATCH_QUEUE_SERIAL);

    videoSession->session = session;
    videoSession->input = input;
    videoSession->output = output;
    videoSession->delegate = delegate;
    videoSession->queue = queue;

    return videoSession;
}

void video_session_destroy(struct VideoSession* session) {
    if (!session) {
        return;
    }

    if (session->session.running) {
        [session->session stopRunning];
    }

    // malloc した struct に載せた ObjC オブジェクトは ARC が参照カウント管理する。
    // セッションから入出力を外してから free し、malloc 領域だけを解放する（Instruments でリーク有無を確認すること）。
    [session->session removeInput:session->input];
    [session->session removeOutput:session->output];

    free(session);
}

int video_session_start(struct VideoSession* session, FrameCallback callback, void* user_data) {
    if (!session || !callback) {
        return -1;
    }

    session->delegate.callback = callback;
    session->delegate.userData = user_data;

    [session->output setSampleBufferDelegate:session->delegate queue:session->queue];
    [session->session startRunning];

    if (!session->session.running) {
        return -2;
    }

    return 0;
}

void video_session_stop(struct VideoSession* session) {
    if (!session) {
        return;
    }

    [session->output setSampleBufferDelegate:nil queue:nil];

    // delegate を nil にしても、既に queue 上で実行中のコールバックは完了まで走る。
    // dispatch_sync で空ブロックを投入し、先行する全ブロックの完了を待つことで
    // 以降 userData (CaptureContext) へのアクセスが発生しないことを保証する。
    if (session->queue) {
        dispatch_sync(session->queue, ^{});
    }

    [session->session stopRunning];
}
