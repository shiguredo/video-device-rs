use std::marker::PhantomData;

use crate::sys as ffi;

use crate::error::{Error, Result};
use crate::stream::StreamConfiguration;

/// 設定のバリデーション結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigStatus {
    /// 設定はそのまま有効
    Valid,
    /// 設定は調整された
    Adjusted,
    /// 設定は無効
    Invalid,
}

/// カメラ設定
///
/// Camera::generate_configuration() で生成される。
/// unique_ptr から取り出した所有型で、Drop で destroy する。
pub struct CameraConfiguration {
    pub(crate) ptr: *mut ffi::lc_camera_configuration_t,
}

// CameraConfiguration は内部で排他的に管理されるため Send + Sync は安全
unsafe impl Send for CameraConfiguration {}

impl CameraConfiguration {
    pub(crate) fn from_raw(ptr: *mut ffi::lc_camera_configuration_t) -> Self {
        Self { ptr }
    }

    /// 設定をバリデートする
    pub fn validate(&mut self) -> Result<ConfigStatus> {
        let status = unsafe { ffi::lc_camera_configuration_validate(self.ptr) };
        match status {
            ffi::lc_config_status_t_LC_CONFIG_STATUS_VALID => Ok(ConfigStatus::Valid),
            ffi::lc_config_status_t_LC_CONFIG_STATUS_ADJUSTED => Ok(ConfigStatus::Adjusted),
            _ => Err(Error::InvalidConfig),
        }
    }

    /// ストリーム設定の数を返す
    pub fn size(&self) -> usize {
        unsafe { ffi::lc_camera_configuration_size(self.ptr) }
    }

    /// インデックスでストリーム設定を取得する
    pub fn at(&mut self, index: u32) -> Result<StreamConfiguration<'_>> {
        let sc_ptr = unsafe { ffi::lc_camera_configuration_at(self.ptr, index) };
        if sc_ptr.is_null() {
            return Err(Error::IndexOutOfRange);
        }
        Ok(StreamConfiguration {
            ptr: sc_ptr,
            _phantom: PhantomData,
        })
    }
}

impl Drop for CameraConfiguration {
    fn drop(&mut self) {
        unsafe { ffi::lc_camera_configuration_destroy(self.ptr) };
    }
}
