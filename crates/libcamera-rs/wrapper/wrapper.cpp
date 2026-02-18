#include "wrapper.h"

#include <libcamera/camera.h>
#include <libcamera/camera_manager.h>
#include <libcamera/controls.h>
#include <libcamera/framebuffer.h>
#include <libcamera/framebuffer_allocator.h>
#include <libcamera/request.h>
#include <libcamera/stream.h>

#include <cstring>
#include <memory>
#include <vector>

using namespace libcamera;

/* 内部構造体 */

struct RequestCallbackWrapper {
    lc_request_completed_cb cb = nullptr;
    void *user_data = nullptr;
};

struct lc_camera_manager {
    std::unique_ptr<CameraManager> cm;
};

struct lc_camera {
    std::shared_ptr<Camera> camera;
    RequestCallbackWrapper callback;
};

/* CameraConfiguration は unique_ptr::release() で raw ポインタとして返す */
struct lc_camera_configuration {
    CameraConfiguration *config;
};

/* StreamConfiguration は CameraConfiguration 内の参照 */
struct lc_stream_configuration {
    StreamConfiguration *sc;
};

/* Request は unique_ptr::release() で返す */
struct lc_request {
    Request *request;
};

/* FrameBuffer は Allocator が所有するバッファへのポインタ */
struct lc_frame_buffer {
    FrameBuffer *fb;
};

struct lc_frame_buffer_allocator {
    std::unique_ptr<FrameBufferAllocator> alloc;
};

/* Stream は Camera 内部データへの不透明参照 */
struct lc_stream {
    Stream *stream;
};

/* ControlList は owned フラグ付き */
struct lc_control_list {
    ControlList *list;
    bool owned;
};

/* ヘルパー: StreamRole 変換 */
static StreamRole to_stream_role(lc_stream_role_t role) {
    switch (role) {
    case LC_STREAM_ROLE_RAW:
        return StreamRole::Raw;
    case LC_STREAM_ROLE_STILL_CAPTURE:
        return StreamRole::StillCapture;
    case LC_STREAM_ROLE_VIDEO_RECORDING:
        return StreamRole::VideoRecording;
    case LC_STREAM_ROLE_VIEWFINDER:
        return StreamRole::Viewfinder;
    default:
        return StreamRole::Viewfinder;
    }
}

/* === CameraManager === */

lc_camera_manager_t *lc_camera_manager_create(void) {
    auto mgr = new lc_camera_manager();
    mgr->cm = std::make_unique<CameraManager>();
    return mgr;
}

void lc_camera_manager_destroy(lc_camera_manager_t *mgr) {
    delete mgr;
}

int lc_camera_manager_start(lc_camera_manager_t *mgr) {
    return mgr->cm->start();
}

void lc_camera_manager_stop(lc_camera_manager_t *mgr) {
    mgr->cm->stop();
}

size_t lc_camera_manager_cameras_count(const lc_camera_manager_t *mgr) {
    return mgr->cm->cameras().size();
}

lc_camera_t *lc_camera_manager_get_camera(lc_camera_manager_t *mgr,
                                           size_t index) {
    auto cameras = mgr->cm->cameras();
    if (index >= cameras.size()) {
        return nullptr;
    }
    auto cam = new lc_camera();
    cam->camera = cameras[index];
    return cam;
}

lc_camera_t *lc_camera_manager_get_camera_by_id(lc_camera_manager_t *mgr,
                                                  const char *id) {
    auto camera = mgr->cm->get(id);
    if (!camera) {
        return nullptr;
    }
    auto cam = new lc_camera();
    cam->camera = camera;
    return cam;
}

/* === Camera === */

const char *lc_camera_id(const lc_camera_t *cam) {
    return cam->camera->id().c_str();
}

int lc_camera_acquire(lc_camera_t *cam) {
    return cam->camera->acquire();
}

int lc_camera_release(lc_camera_t *cam) {
    return cam->camera->release();
}

void lc_camera_release_ref(lc_camera_t *cam) {
    delete cam;
}

lc_camera_configuration_t *lc_camera_generate_configuration(
    lc_camera_t *cam, const lc_stream_role_t *roles, size_t roles_count) {
    std::vector<StreamRole> cpp_roles;
    cpp_roles.reserve(roles_count);
    for (size_t i = 0; i < roles_count; i++) {
        cpp_roles.push_back(to_stream_role(roles[i]));
    }
    auto config = cam->camera->generateConfiguration(cpp_roles);
    if (!config) {
        return nullptr;
    }
    auto lc_config = new lc_camera_configuration();
    lc_config->config = config.release();
    return lc_config;
}

int lc_camera_configure(lc_camera_t *cam,
                         lc_camera_configuration_t *config) {
    return cam->camera->configure(config->config);
}

lc_request_t *lc_camera_create_request(lc_camera_t *cam, uint64_t cookie) {
    auto request = cam->camera->createRequest(cookie);
    if (!request) {
        return nullptr;
    }
    auto lc_req = new lc_request();
    lc_req->request = request.release();
    return lc_req;
}

int lc_camera_queue_request(lc_camera_t *cam, lc_request_t *request) {
    return cam->camera->queueRequest(request->request);
}

int lc_camera_start(lc_camera_t *cam) {
    return cam->camera->start();
}

int lc_camera_stop(lc_camera_t *cam) {
    return cam->camera->stop();
}

void lc_camera_connect_request_completed(lc_camera_t *cam,
                                         lc_request_completed_cb cb,
                                         void *user_data) {
    cam->callback.cb = cb;
    cam->callback.user_data = user_data;

    cam->camera->requestCompleted.connect(cam, [cam](Request *request) {
        if (cam->callback.cb) {
            /* Request をラップして渡す (所有権は渡さない) */
            lc_request_t lc_req;
            lc_req.request = request;
            cam->callback.cb(cam->callback.user_data, &lc_req);
        }
    });
}

void lc_camera_disconnect_request_completed(lc_camera_t *cam) {
    cam->camera->requestCompleted.disconnect(cam);
    cam->callback.cb = nullptr;
    cam->callback.user_data = nullptr;
}

/* === CameraConfiguration === */

void lc_camera_configuration_destroy(lc_camera_configuration_t *config) {
    delete config->config;
    delete config;
}

size_t lc_camera_configuration_size(const lc_camera_configuration_t *config) {
    return config->config->size();
}

lc_stream_configuration_t *lc_camera_configuration_at(
    lc_camera_configuration_t *config, unsigned int index) {
    if (index >= config->config->size()) {
        return nullptr;
    }
    auto sc = new lc_stream_configuration();
    sc->sc = &config->config->at(index);
    return sc;
}

lc_config_status_t lc_camera_configuration_validate(
    lc_camera_configuration_t *config) {
    auto status = config->config->validate();
    switch (status) {
    case CameraConfiguration::Valid:
        return LC_CONFIG_STATUS_VALID;
    case CameraConfiguration::Adjusted:
        return LC_CONFIG_STATUS_ADJUSTED;
    case CameraConfiguration::Invalid:
        return LC_CONFIG_STATUS_INVALID;
    default:
        return LC_CONFIG_STATUS_INVALID;
    }
}

/* === StreamConfiguration === */

void lc_stream_configuration_destroy(lc_stream_configuration_t *sc) {
    delete sc;
}

lc_pixel_format_t lc_stream_configuration_get_pixel_format(
    const lc_stream_configuration_t *sc) {
    lc_pixel_format_t fmt;
    fmt.fourcc = sc->sc->pixelFormat.fourcc();
    fmt.modifier = sc->sc->pixelFormat.modifier();
    return fmt;
}

void lc_stream_configuration_set_pixel_format(lc_stream_configuration_t *sc,
                                              lc_pixel_format_t fmt) {
    sc->sc->pixelFormat = PixelFormat(fmt.fourcc, fmt.modifier);
}

lc_size_t lc_stream_configuration_get_size(
    const lc_stream_configuration_t *sc) {
    lc_size_t size;
    size.width = sc->sc->size.width;
    size.height = sc->sc->size.height;
    return size;
}

void lc_stream_configuration_set_size(lc_stream_configuration_t *sc,
                                      lc_size_t size) {
    sc->sc->size.width = size.width;
    sc->sc->size.height = size.height;
}

unsigned int lc_stream_configuration_get_stride(
    const lc_stream_configuration_t *sc) {
    return sc->sc->stride;
}

unsigned int lc_stream_configuration_get_frame_size(
    const lc_stream_configuration_t *sc) {
    return sc->sc->frameSize;
}

unsigned int lc_stream_configuration_get_buffer_count(
    const lc_stream_configuration_t *sc) {
    return sc->sc->bufferCount;
}

void lc_stream_configuration_set_buffer_count(lc_stream_configuration_t *sc,
                                              unsigned int count) {
    sc->sc->bufferCount = count;
}

lc_stream_t *lc_stream_configuration_stream(
    const lc_stream_configuration_t *sc) {
    auto stream = sc->sc->stream();
    if (!stream) {
        return nullptr;
    }
    auto lc_st = new lc_stream();
    lc_st->stream = stream;
    return lc_st;
}

/* === FrameBufferAllocator === */

lc_frame_buffer_allocator_t *lc_frame_buffer_allocator_create(
    lc_camera_t *cam) {
    auto alloc = new lc_frame_buffer_allocator();
    alloc->alloc =
        std::make_unique<FrameBufferAllocator>(cam->camera);
    return alloc;
}

void lc_frame_buffer_allocator_destroy(lc_frame_buffer_allocator_t *alloc) {
    delete alloc;
}

int lc_frame_buffer_allocator_allocate(lc_frame_buffer_allocator_t *alloc,
                                       lc_stream_t *stream) {
    return alloc->alloc->allocate(stream->stream);
}

int lc_frame_buffer_allocator_free(lc_frame_buffer_allocator_t *alloc,
                                   lc_stream_t *stream) {
    return alloc->alloc->free(stream->stream);
}

size_t lc_frame_buffer_allocator_buffers_count(
    const lc_frame_buffer_allocator_t *alloc, lc_stream_t *stream) {
    const auto &buffers = alloc->alloc->buffers(stream->stream);
    return buffers.size();
}

lc_frame_buffer_t *lc_frame_buffer_allocator_get_buffer(
    const lc_frame_buffer_allocator_t *alloc, lc_stream_t *stream,
    size_t index) {
    const auto &buffers = alloc->alloc->buffers(stream->stream);
    if (index >= buffers.size()) {
        return nullptr;
    }
    auto fb = new lc_frame_buffer();
    fb->fb = buffers[index].get();
    return fb;
}

/* === Request === */

void lc_request_destroy(lc_request_t *request) {
    delete request->request;
    delete request;
}

void lc_request_reuse(lc_request_t *request) {
    request->request->reuse(Request::ReuseBuffers);
}

int lc_request_add_buffer(lc_request_t *request, lc_stream_t *stream,
                          lc_frame_buffer_t *buffer) {
    return request->request->addBuffer(stream->stream, buffer->fb);
}

lc_request_status_t lc_request_status(const lc_request_t *request) {
    switch (request->request->status()) {
    case Request::RequestPending:
        return LC_REQUEST_STATUS_PENDING;
    case Request::RequestComplete:
        return LC_REQUEST_STATUS_COMPLETE;
    case Request::RequestCancelled:
        return LC_REQUEST_STATUS_CANCELLED;
    default:
        return LC_REQUEST_STATUS_CANCELLED;
    }
}

uint64_t lc_request_cookie(const lc_request_t *request) {
    return request->request->cookie();
}

uint32_t lc_request_sequence(const lc_request_t *request) {
    return request->request->sequence();
}

lc_control_list_t *lc_request_controls(lc_request_t *request) {
    auto cl = new lc_control_list();
    cl->list = &request->request->controls();
    cl->owned = false;
    return cl;
}

lc_control_list_t *lc_request_metadata(const lc_request_t *request) {
    auto cl = new lc_control_list();
    cl->list = const_cast<ControlList *>(&request->request->metadata());
    cl->owned = false;
    return cl;
}

lc_frame_buffer_t *lc_request_find_buffer(const lc_request_t *request,
                                          lc_stream_t *stream) {
    auto *fb = request->request->findBuffer(stream->stream);
    if (!fb) {
        return nullptr;
    }
    auto lc_fb = new lc_frame_buffer();
    lc_fb->fb = fb;
    return lc_fb;
}

/* === FrameBuffer === */

void lc_frame_buffer_ref_destroy(lc_frame_buffer_t *fb) {
    delete fb;
}

size_t lc_frame_buffer_planes_count(const lc_frame_buffer_t *fb) {
    return fb->fb->planes().size();
}

lc_frame_buffer_plane_t lc_frame_buffer_get_plane(
    const lc_frame_buffer_t *fb, size_t index) {
    lc_frame_buffer_plane_t plane;
    const auto &planes = fb->fb->planes();
    if (index >= planes.size()) {
        plane.fd = -1;
        plane.offset = 0;
        plane.length = 0;
        return plane;
    }
    plane.fd = planes[index].fd.get();
    plane.offset = planes[index].offset;
    plane.length = planes[index].length;
    return plane;
}

lc_frame_metadata_t lc_frame_buffer_metadata(const lc_frame_buffer_t *fb) {
    lc_frame_metadata_t meta;
    const auto &md = fb->fb->metadata();
    meta.status = static_cast<int>(md.status);
    meta.sequence = md.sequence;
    meta.timestamp = md.timestamp;
    return meta;
}

/* === Stream === */

void lc_stream_ref_destroy(lc_stream_t *stream) {
    delete stream;
}

/* === ControlList === */

void lc_control_list_ref_destroy(lc_control_list_t *list) {
    delete list;
}

size_t lc_control_list_size(const lc_control_list_t *list) {
    return list->list->size();
}

bool lc_control_list_contains(const lc_control_list_t *list, unsigned int id) {
    return list->list->contains(id);
}

bool lc_control_list_get_bool(const lc_control_list_t *list, unsigned int id,
                              bool *out) {
    if (!list->list->contains(id)) {
        return false;
    }
    const auto &val = list->list->get(id);
    if (val.type() != ControlTypeBool) {
        return false;
    }
    *out = val.get<bool>();
    return true;
}

bool lc_control_list_get_int32(const lc_control_list_t *list, unsigned int id,
                               int32_t *out) {
    if (!list->list->contains(id)) {
        return false;
    }
    const auto &val = list->list->get(id);
    if (val.type() != ControlTypeInteger32) {
        return false;
    }
    *out = val.get<int32_t>();
    return true;
}

bool lc_control_list_get_int64(const lc_control_list_t *list, unsigned int id,
                               int64_t *out) {
    if (!list->list->contains(id)) {
        return false;
    }
    const auto &val = list->list->get(id);
    if (val.type() != ControlTypeInteger64) {
        return false;
    }
    *out = val.get<int64_t>();
    return true;
}

bool lc_control_list_get_float(const lc_control_list_t *list, unsigned int id,
                               float *out) {
    if (!list->list->contains(id)) {
        return false;
    }
    const auto &val = list->list->get(id);
    if (val.type() != ControlTypeFloat) {
        return false;
    }
    *out = val.get<float>();
    return true;
}

bool lc_control_list_get_rectangle(const lc_control_list_t *list,
                                   unsigned int id, lc_rectangle_t *out) {
    if (!list->list->contains(id)) {
        return false;
    }
    const auto &val = list->list->get(id);
    if (val.type() != ControlTypeRectangle) {
        return false;
    }
    auto rect = val.get<Rectangle>();
    out->x = rect.x;
    out->y = rect.y;
    out->width = rect.width;
    out->height = rect.height;
    return true;
}

bool lc_control_list_get_float_array(const lc_control_list_t *list,
                                     unsigned int id, const float **out,
                                     size_t *count) {
    if (!list->list->contains(id)) {
        return false;
    }
    const auto &val = list->list->get(id);
    if (val.type() != ControlTypeFloat || !val.isArray()) {
        return false;
    }
    auto data = val.data();
    *out = reinterpret_cast<const float *>(data.data());
    *count = val.numElements();
    return true;
}

bool lc_control_list_get_int32_array(const lc_control_list_t *list,
                                     unsigned int id, const int32_t **out,
                                     size_t *count) {
    if (!list->list->contains(id)) {
        return false;
    }
    const auto &val = list->list->get(id);
    if (val.type() != ControlTypeInteger32 || !val.isArray()) {
        return false;
    }
    auto data = val.data();
    *out = reinterpret_cast<const int32_t *>(data.data());
    *count = val.numElements();
    return true;
}

void lc_control_list_set_bool(lc_control_list_t *list, unsigned int id,
                              bool value) {
    ControlValue cv;
    cv.set<bool>(value);
    list->list->set(id, cv);
}

void lc_control_list_set_int32(lc_control_list_t *list, unsigned int id,
                               int32_t value) {
    ControlValue cv;
    cv.set<int32_t>(value);
    list->list->set(id, cv);
}

void lc_control_list_set_int64(lc_control_list_t *list, unsigned int id,
                               int64_t value) {
    ControlValue cv;
    cv.set<int64_t>(value);
    list->list->set(id, cv);
}

void lc_control_list_set_float(lc_control_list_t *list, unsigned int id,
                               float value) {
    ControlValue cv;
    cv.set<float>(value);
    list->list->set(id, cv);
}

void lc_control_list_set_rectangle(lc_control_list_t *list, unsigned int id,
                                   lc_rectangle_t value) {
    Rectangle rect(value.x, value.y, value.width, value.height);
    ControlValue cv;
    cv.set<Rectangle>(rect);
    list->list->set(id, cv);
}

void lc_control_list_set_float_array(lc_control_list_t *list, unsigned int id,
                                     const float *values, size_t count) {
    ControlValue cv(Span<const float>(values, count));
    list->list->set(id, cv);
}

void lc_control_list_set_int32_array(lc_control_list_t *list, unsigned int id,
                                     const int32_t *values, size_t count) {
    ControlValue cv(Span<const int32_t>(values, count));
    list->list->set(id, cv);
}
