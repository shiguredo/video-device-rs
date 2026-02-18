use crate::sys as ffi;

use crate::configuration::CameraConfiguration;
use crate::error::{Error, Result};
use crate::request::{CompletedRequest, Request};
use crate::stream::StreamRole;

/// コールバックコンテキスト
struct CallbackContext {
    closure: Box<dyn Fn(CompletedRequest<'_>) + Send + Sync>,
}

/// カメラ
///
/// shared_ptr<Camera> を保持する独立した所有型。
/// Drop で ref を解放する。
pub struct Camera {
    pub(crate) ptr: *mut ffi::lc_camera_t,
    _callback_ctx: Option<*mut CallbackContext>,
}

// Camera は内部で排他的に管理されるため Send + Sync は安全
unsafe impl Send for Camera {}
unsafe impl Sync for Camera {}

/// extern "C" トランポリン関数
extern "C" fn request_completed_trampoline(
    user_data: *mut std::ffi::c_void,
    request: *mut ffi::lc_request_t,
) {
    let ctx = unsafe { &*(user_data as *const CallbackContext) };
    let completed = CompletedRequest::from_raw(request);
    (ctx.closure)(completed);
}

impl Camera {
    pub(crate) fn from_raw(ptr: *mut ffi::lc_camera_t) -> Self {
        Self {
            ptr,
            _callback_ctx: None,
        }
    }

    /// カメラの ID を返す
    pub fn id(&self) -> &str {
        let c_str = unsafe { ffi::lc_camera_id(self.ptr) };
        unsafe { std::ffi::CStr::from_ptr(c_str) }
            .to_str()
            .unwrap_or("")
    }

    /// カメラを取得 (acquire) する
    pub fn acquire(&self) -> Result<()> {
        let ret = unsafe { ffi::lc_camera_acquire(self.ptr) };
        if ret != 0 {
            return Err(Error::AcquireFailed);
        }
        Ok(())
    }

    /// カメラを解放 (release) する
    pub fn release(&self) -> Result<()> {
        let ret = unsafe { ffi::lc_camera_release(self.ptr) };
        if ret != 0 {
            return Err(Error::AcquireFailed);
        }
        Ok(())
    }

    /// ストリームロールから設定を生成する
    pub fn generate_configuration(&self, roles: &[StreamRole]) -> Result<CameraConfiguration> {
        let raw_roles: Vec<ffi::lc_stream_role_t> = roles.iter().map(|r| r.to_raw()).collect();
        let config_ptr = unsafe {
            ffi::lc_camera_generate_configuration(self.ptr, raw_roles.as_ptr(), raw_roles.len())
        };
        if config_ptr.is_null() {
            return Err(Error::GenerateConfigFailed);
        }
        Ok(CameraConfiguration::from_raw(config_ptr))
    }

    /// カメラを設定する
    pub fn configure(&self, config: &mut CameraConfiguration) -> Result<()> {
        let ret = unsafe { ffi::lc_camera_configure(self.ptr, config.ptr) };
        if ret != 0 {
            return Err(Error::ConfigureFailed);
        }
        Ok(())
    }

    /// リクエストを作成する
    pub fn create_request(&self, cookie: u64) -> Result<Request> {
        let req_ptr = unsafe { ffi::lc_camera_create_request(self.ptr, cookie) };
        if req_ptr.is_null() {
            return Err(Error::CreateRequestFailed);
        }
        Ok(Request::from_raw(req_ptr))
    }

    /// リクエストをキューに入れる
    pub fn queue_request(&self, request: &Request) -> Result<()> {
        let ret = unsafe { ffi::lc_camera_queue_request(self.ptr, request.ptr) };
        if ret != 0 {
            return Err(Error::QueueRequestFailed);
        }
        Ok(())
    }

    /// カメラのキャプチャを開始する
    pub fn start(&self) -> Result<()> {
        let ret = unsafe { ffi::lc_camera_start(self.ptr) };
        if ret != 0 {
            return Err(Error::CameraStartFailed);
        }
        Ok(())
    }

    /// カメラのキャプチャを停止する
    pub fn stop(&self) -> Result<()> {
        let ret = unsafe { ffi::lc_camera_stop(self.ptr) };
        if ret != 0 {
            return Err(Error::CameraStopFailed);
        }
        Ok(())
    }

    /// リクエスト完了コールバックを設定する
    pub fn on_request_completed<F>(&mut self, callback: F)
    where
        F: Fn(CompletedRequest<'_>) + Send + Sync + 'static,
    {
        // 既存のコールバックがあれば先に切断する
        self.disconnect_request_completed();

        let ctx = Box::new(CallbackContext {
            closure: Box::new(callback),
        });
        let ctx_ptr = Box::into_raw(ctx);
        self._callback_ctx = Some(ctx_ptr);

        unsafe {
            ffi::lc_camera_connect_request_completed(
                self.ptr,
                Some(request_completed_trampoline),
                ctx_ptr as *mut std::ffi::c_void,
            );
        }
    }

    /// リクエスト完了コールバックを切断する
    fn disconnect_request_completed(&mut self) {
        if let Some(ctx_ptr) = self._callback_ctx.take() {
            unsafe {
                ffi::lc_camera_disconnect_request_completed(self.ptr);
                drop(Box::from_raw(ctx_ptr));
            }
        }
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        self.disconnect_request_completed();
        unsafe { ffi::lc_camera_release_ref(self.ptr) };
    }
}
