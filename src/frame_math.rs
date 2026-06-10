/// NV12 の Y / UV プレーンのバイト長を計算する。負のストライドやオーバーフロー時は None。
#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
pub(crate) fn nv12_plane_sizes(stride: i32, stride_uv: i32, height: i32) -> Option<(usize, usize)> {
    if stride <= 0 || stride_uv <= 0 || height <= 0 {
        return None;
    }
    let h = height as usize;
    let y = (stride as usize).checked_mul(h)?;
    let uv_h = h.div_ceil(2);
    let uv = (stride_uv as usize).checked_mul(uv_h)?;
    Some((y, uv))
}

/// I420 の Y / 連結 UV（U 行のあと V 行）のバイト長を計算する。
///
/// macOS `video_c.m` は `chromaHeight = (height + 1) / 2`、`uvSize = strideUV * chromaHeight * 2`
/// で `calloc` し、`stride_uv` は `(int)strideUV` としてコールバックに渡す。
/// 本関数の UV は `stride_uv * ((height + 1) / 2) * 2`（usize での切り上げ整合）であり、
/// 偶数 `height` では `stride_uv * height` と同値、奇数 `height` では C の `uvSize` と一致する。
/// Linux PipeWire など他経路の I420 では、連結 UV の実バイト数やストライドの解釈がこの式と一致しない場合がある。
#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
pub(crate) fn i420_plane_sizes(stride: i32, stride_uv: i32, height: i32) -> Option<(usize, usize)> {
    if stride <= 0 || stride_uv <= 0 || height <= 0 {
        return None;
    }
    let h = height as usize;
    let y = (stride as usize).checked_mul(h)?;
    let chroma_h = h.div_ceil(2);
    let uv = (stride_uv as usize).checked_mul(chroma_h)?.checked_mul(2)?;
    Some((y, uv))
}

/// YUY2 の 1 フレーム分のバイト長を計算する。
#[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
pub(crate) fn yuy2_packed_frame_bytes(stride: i32, height: i32) -> Option<usize> {
    if stride <= 0 || height <= 0 {
        return None;
    }
    (stride as usize).checked_mul(height as usize)
}

/// NV12 連続バッファに必要な Y+UV バイト数。
#[cfg(enable_mf)]
pub(crate) fn nv12_packed_frame_bytes(width: i32, height: i32) -> Option<usize> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let y = (width as usize).checked_mul(height as usize)?;
    let uv = (width as usize).checked_mul((height as usize).div_ceil(2))?;
    y.checked_add(uv)
}

/// I420 連結 Y+U+V に必要なバイト数（Y + U + V の合計が Y+Y/2 になる標準レイアウト）。
#[cfg(enable_mf)]
pub(crate) fn i420_packed_frame_bytes(width: i32, height: i32) -> Option<usize> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let y = (width as usize).checked_mul(height as usize)?;
    let uv = ((width as usize).div_ceil(2)).checked_mul((height as usize).div_ceil(2))?;
    y.checked_add(uv.checked_mul(2)?)
}

/// YUY2 の 1 フレーム分のバイト数。
#[cfg(enable_mf)]
pub(crate) fn yuy2_packed_frame_bytes_win(width: i32, height: i32) -> Option<usize> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let stride = width.checked_mul(2)?;
    (stride as usize).checked_mul(height as usize)
}

/// C ABI の `stride` 引数 (i32) を JPEG ペイロード長スロットとして流用する経路。
/// 呼び出し側は `mjpeg_payload_bytes(stride)` の形で呼ぶ。
/// `payload_size <= 0` の場合は `None` を返す。
#[cfg(enable_mjpeg)]
pub(crate) fn mjpeg_payload_bytes(payload_size: i32) -> Option<usize> {
    if payload_size <= 0 {
        return None;
    }
    Some(payload_size as usize)
}

#[cfg(test)]
mod tests {
    #[cfg(enable_mf)]
    use super::{i420_packed_frame_bytes, nv12_packed_frame_bytes, yuy2_packed_frame_bytes_win};
    #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
    use super::{i420_plane_sizes, nv12_plane_sizes, yuy2_packed_frame_bytes};

    #[test]
    #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
    fn nv12_rejects_non_positive_dimensions() {
        assert_eq!(nv12_plane_sizes(0, 4, 480), None);
        assert_eq!(nv12_plane_sizes(4, 0, 480), None);
        assert_eq!(nv12_plane_sizes(4, 4, 0), None);
        assert_eq!(nv12_plane_sizes(-1, 4, 480), None);
    }

    #[test]
    #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
    fn nv12_small_known_sizes() {
        // Y: 4*2=8, UV 行は div_ceil(2,2)=1, UV: 4*1=4
        assert_eq!(nv12_plane_sizes(4, 4, 2), Some((8, 4)));
    }

    #[test]
    #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
    fn nv12_odd_height_uv_rows_use_div_ceil() {
        // height=3 -> uv 行数 2, UV: stride_uv * 2
        assert_eq!(nv12_plane_sizes(8, 8, 3), Some((24, 16)));
    }

    #[test]
    #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
    fn i420_matches_macos_uv_formula() {
        // video_c.m: uvSize = strideUV * chromaHeight * 2, chromaHeight = (height + 1) / 2
        // height=480, stride_uv=320 -> chroma_h=240, uv=320*240*2=153600
        assert_eq!(i420_plane_sizes(640, 320, 480), Some((307_200, 153_600)));
        // 奇数 height=3, stride_uv=4 -> chroma_h=2, uv=4*2*2=16
        assert_eq!(i420_plane_sizes(8, 4, 3), Some((24, 16)));
    }

    #[test]
    #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
    fn i420_rejects_non_positive() {
        assert_eq!(i420_plane_sizes(0, 4, 100), None);
        assert_eq!(i420_plane_sizes(4, 0, 100), None);
        assert_eq!(i420_plane_sizes(4, 4, -1), None);
    }

    #[test]
    #[cfg(any(enable_avf, enable_v4l2, enable_pipewire))]
    fn yuy2_packed_bytes_stride_times_height() {
        assert_eq!(yuy2_packed_frame_bytes(640, 480), Some(307_200));
        assert_eq!(yuy2_packed_frame_bytes(0, 480), None);
        assert_eq!(yuy2_packed_frame_bytes(640, 0), None);
    }

    #[test]
    #[cfg(enable_mf)]
    fn nv12_packed_rejects_non_positive() {
        assert_eq!(nv12_packed_frame_bytes(0, 480), None);
        assert_eq!(nv12_packed_frame_bytes(640, 0), None);
        assert_eq!(nv12_packed_frame_bytes(-1, 480), None);
    }

    #[test]
    #[cfg(enable_mf)]
    fn nv12_packed_known_sizes() {
        // 640*480=307200, UV: 640*240=153600, total=460800
        assert_eq!(nv12_packed_frame_bytes(640, 480), Some(307_200 + 153_600));
    }

    #[test]
    #[cfg(enable_mf)]
    fn i420_packed_known_sizes() {
        // Y: 640*480=307200, U: 320*240=76800, V: 320*240=76800, total=460800
        assert_eq!(
            i420_packed_frame_bytes(640, 480),
            Some(307_200 + 76_800 + 76_800)
        );
    }

    #[test]
    #[cfg(enable_mf)]
    fn yuy2_packed_win_stride_doubled() {
        // YUY2: 2 bytes per pixel, so stride = width * 2
        assert_eq!(yuy2_packed_frame_bytes_win(640, 480), Some(640 * 2 * 480));
        assert_eq!(yuy2_packed_frame_bytes_win(0, 480), None);
    }

    #[test]
    #[cfg(enable_mjpeg)]
    fn mjpeg_rejects_non_positive_payload_size() {
        assert_eq!(super::mjpeg_payload_bytes(0), None);
        assert_eq!(super::mjpeg_payload_bytes(-1), None);
        assert_eq!(super::mjpeg_payload_bytes(i32::MIN), None);
    }

    #[test]
    #[cfg(enable_mjpeg)]
    fn mjpeg_payload_bytes_returns_input() {
        assert_eq!(super::mjpeg_payload_bytes(1024), Some(1024));
        assert_eq!(
            super::mjpeg_payload_bytes(i32::MAX),
            Some(i32::MAX as usize)
        );
    }
}
