use crate::sys as ffi;

/// 矩形
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rectangle {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rectangle {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub(crate) fn from_raw(raw: ffi::lc_rectangle_t) -> Self {
        Self {
            x: raw.x,
            y: raw.y,
            width: raw.width,
            height: raw.height,
        }
    }

    pub(crate) fn to_raw(self) -> ffi::lc_rectangle_t {
        ffi::lc_rectangle_t {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

/// サイズ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub(crate) fn from_raw(raw: ffi::lc_size_t) -> Self {
        Self {
            width: raw.width,
            height: raw.height,
        }
    }

    pub(crate) fn to_raw(self) -> ffi::lc_size_t {
        ffi::lc_size_t {
            width: self.width,
            height: self.height,
        }
    }
}

/// 座標点
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}
