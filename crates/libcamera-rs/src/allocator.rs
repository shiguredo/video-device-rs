use crate::sys as ffi;

use crate::camera::Camera;
use crate::error::{Error, Result};
use crate::frame_buffer::FrameBuffer;
use crate::stream::Stream;

/// フレームバッファアロケーター
///
/// カメラ用のフレームバッファを割り当てる。
/// Drop で destroy する。
pub struct FrameBufferAllocator {
    ptr: *mut ffi::lc_frame_buffer_allocator_t,
}

// FrameBufferAllocator は内部で排他的に管理されるため Send は安全
unsafe impl Send for FrameBufferAllocator {}

impl FrameBufferAllocator {
    /// アロケーターを作成する
    pub fn new(camera: &Camera) -> Self {
        let ptr = unsafe { ffi::lc_frame_buffer_allocator_create(camera.ptr) };
        Self { ptr }
    }

    /// ストリーム用のバッファを割り当てる
    pub fn allocate(&self, stream: &Stream) -> Result<usize> {
        let ret = unsafe { ffi::lc_frame_buffer_allocator_allocate(self.ptr, stream.ptr) };
        if ret < 0 {
            return Err(Error::AllocateFailed);
        }
        Ok(ret as usize)
    }

    /// ストリーム用のバッファを解放する
    pub fn free(&self, stream: &Stream) -> Result<()> {
        let ret = unsafe { ffi::lc_frame_buffer_allocator_free(self.ptr, stream.ptr) };
        if ret < 0 {
            return Err(Error::FreeFailed);
        }
        Ok(())
    }

    /// ストリームのバッファ数を返す
    pub fn buffers_count(&self, stream: &Stream) -> usize {
        unsafe { ffi::lc_frame_buffer_allocator_buffers_count(self.ptr, stream.ptr) }
    }

    /// ストリームの指定インデックスのバッファを取得する
    pub fn get_buffer(&self, stream: &Stream, index: usize) -> Result<FrameBuffer> {
        let fb_ptr =
            unsafe { ffi::lc_frame_buffer_allocator_get_buffer(self.ptr, stream.ptr, index) };
        if fb_ptr.is_null() {
            return Err(Error::IndexOutOfRange);
        }
        Ok(FrameBuffer::from_raw(fb_ptr))
    }
}

impl Drop for FrameBufferAllocator {
    fn drop(&mut self) {
        unsafe { ffi::lc_frame_buffer_allocator_destroy(self.ptr) };
    }
}
