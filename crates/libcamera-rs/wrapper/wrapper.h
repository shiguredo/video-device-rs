#ifndef LIBCAMERA_C_WRAPPER_H
#define LIBCAMERA_C_WRAPPER_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* 不透明ポインタ型 */
typedef struct lc_camera_manager lc_camera_manager_t;
typedef struct lc_camera lc_camera_t;
typedef struct lc_camera_configuration lc_camera_configuration_t;
typedef struct lc_stream_configuration lc_stream_configuration_t;
typedef struct lc_request lc_request_t;
typedef struct lc_frame_buffer lc_frame_buffer_t;
typedef struct lc_frame_buffer_allocator lc_frame_buffer_allocator_t;
typedef struct lc_stream lc_stream_t;
typedef struct lc_control_list lc_control_list_t;

/* POD 型 */
typedef struct {
  int x;
  int y;
  unsigned int width;
  unsigned int height;
} lc_rectangle_t;

typedef struct {
  unsigned int width;
  unsigned int height;
} lc_size_t;

typedef struct {
  int x;
  int y;
} lc_point_t;

typedef struct {
  uint32_t fourcc;
  uint64_t modifier;
} lc_pixel_format_t;

typedef struct {
  int fd;
  unsigned int offset;
  unsigned int length;
} lc_frame_buffer_plane_t;

typedef struct {
  int status;
  unsigned int sequence;
  uint64_t timestamp;
} lc_frame_metadata_t;

/* 列挙型 */
typedef enum {
  LC_STREAM_ROLE_RAW = 0,
  LC_STREAM_ROLE_STILL_CAPTURE = 1,
  LC_STREAM_ROLE_VIDEO_RECORDING = 2,
  LC_STREAM_ROLE_VIEWFINDER = 3,
} lc_stream_role_t;

typedef enum {
  LC_CONFIG_STATUS_VALID = 0,
  LC_CONFIG_STATUS_ADJUSTED = 1,
  LC_CONFIG_STATUS_INVALID = 2,
} lc_config_status_t;

typedef enum {
  LC_REQUEST_STATUS_PENDING = 0,
  LC_REQUEST_STATUS_COMPLETE = 1,
  LC_REQUEST_STATUS_CANCELLED = 2,
} lc_request_status_t;

typedef enum {
  LC_FRAME_STATUS_SUCCESS = 0,
  LC_FRAME_STATUS_ERROR = 1,
  LC_FRAME_STATUS_CANCELLED = 2,
  LC_FRAME_STATUS_STARTUP = 3,
} lc_frame_status_t;

typedef enum {
  LC_CONTROL_TYPE_NONE = 0,
  LC_CONTROL_TYPE_BOOL = 1,
  LC_CONTROL_TYPE_BYTE = 2,
  LC_CONTROL_TYPE_UNSIGNED16 = 3,
  LC_CONTROL_TYPE_UNSIGNED32 = 4,
  LC_CONTROL_TYPE_INT32 = 5,
  LC_CONTROL_TYPE_INT64 = 6,
  LC_CONTROL_TYPE_FLOAT = 7,
  LC_CONTROL_TYPE_STRING = 8,
  LC_CONTROL_TYPE_RECTANGLE = 9,
  LC_CONTROL_TYPE_SIZE = 10,
  LC_CONTROL_TYPE_POINT = 11,
} lc_control_type_t;

/* コールバック */
typedef void (*lc_request_completed_cb)(void* user_data, lc_request_t* request);

/* === CameraManager === */
lc_camera_manager_t* lc_camera_manager_create(void);
void lc_camera_manager_destroy(lc_camera_manager_t* mgr);
int lc_camera_manager_start(lc_camera_manager_t* mgr);
void lc_camera_manager_stop(lc_camera_manager_t* mgr);
size_t lc_camera_manager_cameras_count(const lc_camera_manager_t* mgr);
lc_camera_t* lc_camera_manager_get_camera(lc_camera_manager_t* mgr,
                                          size_t index);
lc_camera_t* lc_camera_manager_get_camera_by_id(lc_camera_manager_t* mgr,
                                                const char* id);

/* === Camera === */
const char* lc_camera_id(const lc_camera_t* cam);
int lc_camera_acquire(lc_camera_t* cam);
int lc_camera_release(lc_camera_t* cam);
void lc_camera_release_ref(lc_camera_t* cam);
lc_camera_configuration_t* lc_camera_generate_configuration(
    lc_camera_t* cam,
    const lc_stream_role_t* roles,
    size_t roles_count);
int lc_camera_configure(lc_camera_t* cam, lc_camera_configuration_t* config);
lc_request_t* lc_camera_create_request(lc_camera_t* cam, uint64_t cookie);
int lc_camera_queue_request(lc_camera_t* cam, lc_request_t* request);
int lc_camera_start(lc_camera_t* cam);
int lc_camera_stop(lc_camera_t* cam);
void lc_camera_connect_request_completed(lc_camera_t* cam,
                                         lc_request_completed_cb cb,
                                         void* user_data);
void lc_camera_disconnect_request_completed(lc_camera_t* cam);

/* === CameraConfiguration === */
void lc_camera_configuration_destroy(lc_camera_configuration_t* config);
size_t lc_camera_configuration_size(const lc_camera_configuration_t* config);
lc_stream_configuration_t* lc_camera_configuration_at(
    lc_camera_configuration_t* config,
    unsigned int index);
lc_config_status_t lc_camera_configuration_validate(
    lc_camera_configuration_t* config);

/* === StreamConfiguration === */
void lc_stream_configuration_destroy(lc_stream_configuration_t* sc);
lc_pixel_format_t lc_stream_configuration_get_pixel_format(
    const lc_stream_configuration_t* sc);
void lc_stream_configuration_set_pixel_format(lc_stream_configuration_t* sc,
                                              lc_pixel_format_t fmt);
lc_size_t lc_stream_configuration_get_size(const lc_stream_configuration_t* sc);
void lc_stream_configuration_set_size(lc_stream_configuration_t* sc,
                                      lc_size_t size);
unsigned int lc_stream_configuration_get_stride(
    const lc_stream_configuration_t* sc);
unsigned int lc_stream_configuration_get_frame_size(
    const lc_stream_configuration_t* sc);
unsigned int lc_stream_configuration_get_buffer_count(
    const lc_stream_configuration_t* sc);
void lc_stream_configuration_set_buffer_count(lc_stream_configuration_t* sc,
                                              unsigned int count);
lc_stream_t* lc_stream_configuration_stream(
    const lc_stream_configuration_t* sc);

/* === FrameBufferAllocator === */
lc_frame_buffer_allocator_t* lc_frame_buffer_allocator_create(lc_camera_t* cam);
void lc_frame_buffer_allocator_destroy(lc_frame_buffer_allocator_t* alloc);
int lc_frame_buffer_allocator_allocate(lc_frame_buffer_allocator_t* alloc,
                                       lc_stream_t* stream);
int lc_frame_buffer_allocator_free(lc_frame_buffer_allocator_t* alloc,
                                   lc_stream_t* stream);
size_t lc_frame_buffer_allocator_buffers_count(
    const lc_frame_buffer_allocator_t* alloc,
    lc_stream_t* stream);
lc_frame_buffer_t* lc_frame_buffer_allocator_get_buffer(
    const lc_frame_buffer_allocator_t* alloc,
    lc_stream_t* stream,
    size_t index);

/* === Request === */
void lc_request_destroy(lc_request_t* request);
void lc_request_reuse(lc_request_t* request);
int lc_request_add_buffer(lc_request_t* request,
                          lc_stream_t* stream,
                          lc_frame_buffer_t* buffer);
lc_request_status_t lc_request_status(const lc_request_t* request);
uint64_t lc_request_cookie(const lc_request_t* request);
uint32_t lc_request_sequence(const lc_request_t* request);
lc_control_list_t* lc_request_controls(lc_request_t* request);
lc_control_list_t* lc_request_metadata(const lc_request_t* request);
lc_frame_buffer_t* lc_request_find_buffer(const lc_request_t* request,
                                          lc_stream_t* stream);

/* === FrameBuffer === */
void lc_frame_buffer_ref_destroy(lc_frame_buffer_t* fb);
size_t lc_frame_buffer_planes_count(const lc_frame_buffer_t* fb);
lc_frame_buffer_plane_t lc_frame_buffer_get_plane(const lc_frame_buffer_t* fb,
                                                  size_t index);
lc_frame_metadata_t lc_frame_buffer_metadata(const lc_frame_buffer_t* fb);

/* === Stream === */
void lc_stream_ref_destroy(lc_stream_t* stream);

/* === ControlList === */
void lc_control_list_ref_destroy(lc_control_list_t* list);
size_t lc_control_list_size(const lc_control_list_t* list);
bool lc_control_list_contains(const lc_control_list_t* list, unsigned int id);

bool lc_control_list_get_bool(const lc_control_list_t* list,
                              unsigned int id,
                              bool* out);
bool lc_control_list_get_int32(const lc_control_list_t* list,
                               unsigned int id,
                               int32_t* out);
bool lc_control_list_get_int64(const lc_control_list_t* list,
                               unsigned int id,
                               int64_t* out);
bool lc_control_list_get_float(const lc_control_list_t* list,
                               unsigned int id,
                               float* out);
bool lc_control_list_get_rectangle(const lc_control_list_t* list,
                                   unsigned int id,
                                   lc_rectangle_t* out);
bool lc_control_list_get_float_array(const lc_control_list_t* list,
                                     unsigned int id,
                                     const float** out,
                                     size_t* count);
bool lc_control_list_get_int32_array(const lc_control_list_t* list,
                                     unsigned int id,
                                     const int32_t** out,
                                     size_t* count);

void lc_control_list_set_bool(lc_control_list_t* list,
                              unsigned int id,
                              bool value);
void lc_control_list_set_int32(lc_control_list_t* list,
                               unsigned int id,
                               int32_t value);
void lc_control_list_set_int64(lc_control_list_t* list,
                               unsigned int id,
                               int64_t value);
void lc_control_list_set_float(lc_control_list_t* list,
                               unsigned int id,
                               float value);
void lc_control_list_set_rectangle(lc_control_list_t* list,
                                   unsigned int id,
                                   lc_rectangle_t value);
void lc_control_list_set_float_array(lc_control_list_t* list,
                                     unsigned int id,
                                     const float* values,
                                     size_t count);
void lc_control_list_set_int32_array(lc_control_list_t* list,
                                     unsigned int id,
                                     const int32_t* values,
                                     size_t count);

#ifdef __cplusplus
}
#endif

#endif /* LIBCAMERA_C_WRAPPER_H */
