use std::marker::PhantomData;

use crate::sys as ffi;

use crate::controls::ControlList;
use crate::error::{Error, Result};
use crate::frame_buffer::{FrameBuffer, FrameBufferRef};
use crate::stream::Stream;

/// リクエストステータス
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestStatus {
    Pending,
    Complete,
    Cancelled,
}

/// リクエスト
///
/// unique_ptr から取り出した所有型。
pub struct Request {
    pub(crate) ptr: *mut ffi::lc_request_t,
}

// Request は内部で排他的に管理されるため Send は安全
unsafe impl Send for Request {}

impl Request {
    pub(crate) fn from_raw(ptr: *mut ffi::lc_request_t) -> Self {
        Self { ptr }
    }

    /// バッファを再利用してリクエストをリセットする
    pub fn reuse(&self) {
        unsafe { ffi::lc_request_reuse(self.ptr) };
    }

    /// ストリームにバッファを追加する
    pub fn add_buffer(&self, stream: &Stream, buffer: &FrameBuffer) -> Result<()> {
        let ret = unsafe { ffi::lc_request_add_buffer(self.ptr, stream.ptr, buffer.ptr) };
        if ret != 0 {
            return Err(Error::AddBufferFailed);
        }
        Ok(())
    }

    /// リクエストのステータスを返す
    pub fn status(&self) -> RequestStatus {
        let raw = unsafe { ffi::lc_request_status(self.ptr) };
        match raw {
            ffi::lc_request_status_t_LC_REQUEST_STATUS_PENDING => RequestStatus::Pending,
            ffi::lc_request_status_t_LC_REQUEST_STATUS_COMPLETE => RequestStatus::Complete,
            _ => RequestStatus::Cancelled,
        }
    }

    /// リクエストの cookie を返す
    pub fn cookie(&self) -> u64 {
        unsafe { ffi::lc_request_cookie(self.ptr) }
    }

    /// フレームシーケンス番号を返す
    pub fn sequence(&self) -> u32 {
        unsafe { ffi::lc_request_sequence(self.ptr) }
    }

    /// コントロールリストを返す
    pub fn controls(&self) -> ControlList {
        let cl_ptr = unsafe { ffi::lc_request_controls(self.ptr) };
        ControlList::from_raw(cl_ptr, false)
    }

    /// メタデータのコントロールリストを返す
    pub fn metadata(&self) -> ControlList {
        let cl_ptr = unsafe { ffi::lc_request_metadata(self.ptr) };
        ControlList::from_raw(cl_ptr, false)
    }

    /// ストリームに対応するバッファを検索する
    pub fn find_buffer(&self, stream: &Stream) -> Option<FrameBuffer> {
        let fb_ptr = unsafe { ffi::lc_request_find_buffer(self.ptr, stream.ptr) };
        if fb_ptr.is_null() {
            None
        } else {
            Some(FrameBuffer::from_raw(fb_ptr))
        }
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        unsafe { ffi::lc_request_destroy(self.ptr) };
    }
}

/// コールバック内で使うリクエスト参照
///
/// コールバック内でのみ有効で、所有権を持たない。
pub struct CompletedRequest<'a> {
    ptr: *mut ffi::lc_request_t,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> CompletedRequest<'a> {
    pub(crate) fn from_raw(ptr: *mut ffi::lc_request_t) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }

    /// リクエストのステータスを返す
    pub fn status(&self) -> RequestStatus {
        let raw = unsafe { ffi::lc_request_status(self.ptr) };
        match raw {
            ffi::lc_request_status_t_LC_REQUEST_STATUS_PENDING => RequestStatus::Pending,
            ffi::lc_request_status_t_LC_REQUEST_STATUS_COMPLETE => RequestStatus::Complete,
            _ => RequestStatus::Cancelled,
        }
    }

    /// リクエストの cookie を返す
    pub fn cookie(&self) -> u64 {
        unsafe { ffi::lc_request_cookie(self.ptr) }
    }

    /// フレームシーケンス番号を返す
    pub fn sequence(&self) -> u32 {
        unsafe { ffi::lc_request_sequence(self.ptr) }
    }

    /// メタデータのコントロールリストを返す
    pub fn metadata(&self) -> ControlList {
        let cl_ptr = unsafe { ffi::lc_request_metadata(self.ptr) };
        ControlList::from_raw(cl_ptr, false)
    }

    /// ストリームに対応するバッファ参照を検索する
    pub fn find_buffer(&self, stream: &Stream) -> Option<FrameBufferRef<'a>> {
        let fb_ptr = unsafe { ffi::lc_request_find_buffer(self.ptr, stream.ptr) };
        if fb_ptr.is_null() {
            None
        } else {
            Some(FrameBufferRef {
                ptr: fb_ptr,
                _phantom: PhantomData,
            })
        }
    }
}
