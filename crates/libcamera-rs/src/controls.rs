use crate::sys as ffi;

use crate::error::{Error, Result};
use crate::geometry::Rectangle;

/// コントロールの型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlType {
    None,
    Bool,
    Byte,
    Unsigned16,
    Unsigned32,
    Int32,
    Int64,
    Float,
    String,
    Rectangle,
    Size,
    Point,
}

/// コントロールの方向
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    In,
    Out,
    InOut,
}

/// コントロール ID の定義
#[derive(Debug, Clone)]
pub struct ControlId {
    id: u32,
    name: &'static str,
    control_type: ControlType,
    direction: Direction,
}

impl ControlId {
    /// 新しいコントロール ID を作成する
    pub const fn new(
        id: u32,
        name: &'static str,
        control_type: ControlType,
        direction: Direction,
    ) -> Self {
        Self {
            id,
            name,
            control_type,
            direction,
        }
    }

    /// コントロール ID の数値を返す
    pub fn id(&self) -> u32 {
        self.id
    }

    /// コントロール名を返す
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// コントロールの型を返す
    pub fn control_type(&self) -> ControlType {
        self.control_type
    }

    /// コントロールの方向を返す
    pub fn direction(&self) -> Direction {
        self.direction
    }
}

/// コントロールリスト
///
/// Request の controls() / metadata() から取得する場合は非所有 (owned = false)。
pub struct ControlList {
    ptr: *mut ffi::lc_control_list_t,
    _owned: bool,
}

// ControlList は内部ポインタのラッパーであり libcamera 側でスレッドセーフ
unsafe impl Send for ControlList {}

impl ControlList {
    pub(crate) fn from_raw(ptr: *mut ffi::lc_control_list_t, owned: bool) -> Self {
        Self { ptr, _owned: owned }
    }

    /// コントロールリストのサイズを返す
    pub fn size(&self) -> usize {
        unsafe { ffi::lc_control_list_size(self.ptr) }
    }

    /// 指定 ID のコントロールが含まれているか確認する
    pub fn contains(&self, id: &ControlId) -> bool {
        unsafe { ffi::lc_control_list_contains(self.ptr, id.id()) }
    }

    /// bool 値を取得する
    pub fn get_bool(&self, id: &ControlId) -> Result<bool> {
        let mut out = false;
        let ok = unsafe { ffi::lc_control_list_get_bool(self.ptr, id.id(), &mut out) };
        if ok {
            Ok(out)
        } else {
            Err(Error::ControlGetFailed)
        }
    }

    /// i32 値を取得する
    pub fn get_i32(&self, id: &ControlId) -> Result<i32> {
        let mut out = 0i32;
        let ok = unsafe { ffi::lc_control_list_get_int32(self.ptr, id.id(), &mut out) };
        if ok {
            Ok(out)
        } else {
            Err(Error::ControlGetFailed)
        }
    }

    /// i64 値を取得する
    pub fn get_i64(&self, id: &ControlId) -> Result<i64> {
        let mut out = 0i64;
        let ok = unsafe { ffi::lc_control_list_get_int64(self.ptr, id.id(), &mut out) };
        if ok {
            Ok(out)
        } else {
            Err(Error::ControlGetFailed)
        }
    }

    /// f32 値を取得する
    pub fn get_f32(&self, id: &ControlId) -> Result<f32> {
        let mut out = 0.0f32;
        let ok = unsafe { ffi::lc_control_list_get_float(self.ptr, id.id(), &mut out) };
        if ok {
            Ok(out)
        } else {
            Err(Error::ControlGetFailed)
        }
    }

    /// Rectangle 値を取得する
    pub fn get_rectangle(&self, id: &ControlId) -> Result<Rectangle> {
        let mut out = ffi::lc_rectangle_t::default();
        let ok = unsafe { ffi::lc_control_list_get_rectangle(self.ptr, id.id(), &mut out) };
        if ok {
            Ok(Rectangle::from_raw(out))
        } else {
            Err(Error::ControlGetFailed)
        }
    }

    /// f32 配列を取得する
    pub fn get_f32_array(&self, id: &ControlId) -> Result<&[f32]> {
        let mut ptr = std::ptr::null();
        let mut count = 0usize;
        let ok = unsafe {
            ffi::lc_control_list_get_float_array(self.ptr, id.id(), &mut ptr, &mut count)
        };
        if ok && !ptr.is_null() {
            Ok(unsafe { std::slice::from_raw_parts(ptr, count) })
        } else {
            Err(Error::ControlGetFailed)
        }
    }

    /// i32 配列を取得する
    pub fn get_i32_array(&self, id: &ControlId) -> Result<&[i32]> {
        let mut ptr = std::ptr::null();
        let mut count = 0usize;
        let ok = unsafe {
            ffi::lc_control_list_get_int32_array(self.ptr, id.id(), &mut ptr, &mut count)
        };
        if ok && !ptr.is_null() {
            Ok(unsafe { std::slice::from_raw_parts(ptr, count) })
        } else {
            Err(Error::ControlGetFailed)
        }
    }

    /// bool 値を設定する
    pub fn set_bool(&mut self, id: &ControlId, value: bool) {
        unsafe { ffi::lc_control_list_set_bool(self.ptr, id.id(), value) };
    }

    /// i32 値を設定する
    pub fn set_i32(&mut self, id: &ControlId, value: i32) {
        unsafe { ffi::lc_control_list_set_int32(self.ptr, id.id(), value) };
    }

    /// i64 値を設定する
    pub fn set_i64(&mut self, id: &ControlId, value: i64) {
        unsafe { ffi::lc_control_list_set_int64(self.ptr, id.id(), value) };
    }

    /// f32 値を設定する
    pub fn set_f32(&mut self, id: &ControlId, value: f32) {
        unsafe { ffi::lc_control_list_set_float(self.ptr, id.id(), value) };
    }

    /// Rectangle 値を設定する
    pub fn set_rectangle(&mut self, id: &ControlId, value: Rectangle) {
        unsafe { ffi::lc_control_list_set_rectangle(self.ptr, id.id(), value.to_raw()) };
    }

    /// f32 配列を設定する
    pub fn set_f32_array(&mut self, id: &ControlId, values: &[f32]) {
        unsafe {
            ffi::lc_control_list_set_float_array(self.ptr, id.id(), values.as_ptr(), values.len())
        };
    }

    /// i32 配列を設定する
    pub fn set_i32_array(&mut self, id: &ControlId, values: &[i32]) {
        unsafe {
            ffi::lc_control_list_set_int32_array(self.ptr, id.id(), values.as_ptr(), values.len())
        };
    }
}

impl Drop for ControlList {
    fn drop(&mut self) {
        // owned の場合も非 owned の場合もラッパー構造体は解放する
        unsafe { ffi::lc_control_list_ref_destroy(self.ptr) };
    }
}
