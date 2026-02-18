use crate::sys as ffi;

use crate::camera::Camera;
use crate::error::{Error, Result};

/// カメラマネージャー
///
/// libcamera のカメラマネージャーを管理する。
/// new() で start まで行い、Drop で stop + destroy する。
pub struct CameraManager {
    ptr: *mut ffi::lc_camera_manager_t,
}

// CameraManager は内部で排他的に管理されるため Send + Sync は安全
unsafe impl Send for CameraManager {}
unsafe impl Sync for CameraManager {}

impl CameraManager {
    /// カメラマネージャーを作成して開始する
    pub fn new() -> Result<Self> {
        let ptr = unsafe { ffi::lc_camera_manager_create() };
        if ptr.is_null() {
            return Err(Error::StartFailed);
        }
        let ret = unsafe { ffi::lc_camera_manager_start(ptr) };
        if ret != 0 {
            unsafe { ffi::lc_camera_manager_destroy(ptr) };
            return Err(Error::StartFailed);
        }
        Ok(Self { ptr })
    }

    /// 検出されたカメラの数を返す
    pub fn cameras_count(&self) -> usize {
        unsafe { ffi::lc_camera_manager_cameras_count(self.ptr) }
    }

    /// インデックスでカメラを取得する
    pub fn get_camera(&self, index: usize) -> Result<Camera> {
        let cam_ptr = unsafe { ffi::lc_camera_manager_get_camera(self.ptr, index) };
        if cam_ptr.is_null() {
            return Err(Error::IndexOutOfRange);
        }
        Ok(Camera::from_raw(cam_ptr))
    }

    /// ID でカメラを取得する
    pub fn get_camera_by_id(&self, id: &str) -> Result<Camera> {
        let c_id = std::ffi::CString::new(id).map_err(|_| Error::CameraNotFound)?;
        let cam_ptr = unsafe { ffi::lc_camera_manager_get_camera_by_id(self.ptr, c_id.as_ptr()) };
        if cam_ptr.is_null() {
            return Err(Error::CameraNotFound);
        }
        Ok(Camera::from_raw(cam_ptr))
    }
}

impl Drop for CameraManager {
    fn drop(&mut self) {
        unsafe {
            ffi::lc_camera_manager_stop(self.ptr);
            ffi::lc_camera_manager_destroy(self.ptr);
        }
    }
}
