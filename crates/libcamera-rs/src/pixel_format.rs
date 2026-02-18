use std::fmt;

use crate::sys as ffi;

/// ピクセルフォーマット (fourcc + modifier)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PixelFormat {
    pub fourcc: u32,
    pub modifier: u64,
}

impl PixelFormat {
    pub fn new(fourcc: u32, modifier: u64) -> Self {
        Self { fourcc, modifier }
    }

    pub fn from_fourcc(fourcc: u32) -> Self {
        Self {
            fourcc,
            modifier: 0,
        }
    }

    pub(crate) fn from_raw(raw: ffi::lc_pixel_format_t) -> Self {
        Self {
            fourcc: raw.fourcc,
            modifier: raw.modifier,
        }
    }

    pub(crate) fn to_raw(self) -> ffi::lc_pixel_format_t {
        ffi::lc_pixel_format_t {
            fourcc: self.fourcc,
            modifier: self.modifier,
        }
    }
}

impl fmt::Display for PixelFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.fourcc.to_le_bytes();
        let chars: String = bytes
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '?'
                }
            })
            .collect();
        if self.modifier == 0 {
            write!(f, "{chars}")
        } else {
            write!(f, "{chars}/0x{:x}", self.modifier)
        }
    }
}
