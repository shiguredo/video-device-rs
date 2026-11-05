//! カメラ映像をキャプチャして raw-player でプレビュー表示するサンプル
//!
//! shiguredo_video_device でカメラ映像を取得し、
//! raw_player::video_player::VideoPlayer でリアルタイム表示する。
//!
//! ```
//! cargo run --example camera_preview -- --list-devices
//! cargo run --example camera_preview
//! cargo run --example camera_preview -- --resolution 1080p --fps 60
//! ```

#[cfg(any(enable_avf, enable_v4l2, enable_pipewire, enable_mf))]
mod imp {
    use std::borrow::Cow;
    use std::sync::Once;
    use std::sync::mpsc::sync_channel;
    use std::time::Instant;

    use raw_player::event::KEYCODE_ESCAPE;
    use raw_player::video_player::VideoPlayer;
    use shiguredo_video_device::{
        PixelFormat, VideoCapture, VideoCaptureConfig, VideoDeviceList, VideoFrame, VideoFrameOwned,
    };

    struct Args {
        list_devices: bool,
        video_device_id: Option<String>,
        width: i32,
        height: i32,
        fps: i32,
        duration: Option<f64>,
    }

    /// 解像度指定 ("720p" / "1080p" / "4k" / "WxH") を (幅, 高さ) に変換する。
    fn parse_resolution(s: &str) -> Option<(i32, i32)> {
        match s.to_lowercase().as_str() {
            "4k" | "2160p" => Some((3840, 2160)),
            "1080p" => Some((1920, 1080)),
            "720p" => Some((1280, 720)),
            "540p" => Some((960, 540)),
            _ => {
                let (w, h) = s.split_once('x')?;
                let w = w.parse().ok()?;
                let h = h.parse().ok()?;
                if w > 0 && h > 0 { Some((w, h)) } else { None }
            }
        }
    }

    /// コマンドライン引数をパースする。
    fn parse_args() -> Result<Args, noargs::Error> {
        let mut args = noargs::raw_args();
        args.metadata_mut().app_name = "camera_preview";
        args.metadata_mut().app_description = "Camera preview sample using raw-player";
        noargs::HELP_FLAG.take_help(&mut args);

        let list_devices: bool = noargs::flag("list-devices")
            .doc("List video devices and exit")
            .take(&mut args)
            .is_present();

        let video_device_id: Option<String> = noargs::opt("video-device-id")
            .doc("Video device ID (defaults to the first device)")
            .take(&mut args)
            .present_and_then(|o| Ok::<_, &str>(o.value().to_string()))?;

        let (width, height) = noargs::opt("resolution")
            .doc("Capture resolution: 720p, 1080p, 4k, or WxH")
            .default("720p")
            .take(&mut args)
            .then(|o| {
                parse_resolution(o.value())
                    .ok_or_else(|| format!("invalid resolution: {}", o.value()))
            })?;

        let fps: i32 = noargs::opt("fps")
            .doc("Frame rate")
            .default("30")
            .take(&mut args)
            .then(|o| o.value().parse::<i32>())?;

        let duration: Option<f64> = noargs::opt("duration")
            .doc("Playback duration in seconds (defaults to unlimited)")
            .take(&mut args)
            .present_and_then(|o| o.value().parse::<f64>())?;

        if let Some(help) = args.finish()? {
            print!("{help}");
            std::process::exit(0);
        }

        Ok(Args {
            list_devices,
            video_device_id,
            width,
            height,
            fps,
            duration,
        })
    }

    /// stride を除去してピクセルデータのみを取り出す。
    /// stride == row_bytes の場合はゼロコピーで借用を返す。
    /// 寸法・スライス長が不整合なら空の `Vec` を返しパニックしない。
    fn strip_stride<'a>(
        data: &'a [u8],
        row_bytes: usize,
        height: usize,
        stride: usize,
    ) -> Cow<'a, [u8]> {
        if row_bytes == 0 || height == 0 {
            return Cow::Owned(Vec::new());
        }
        let Some(total_needed) = row_bytes.checked_mul(height) else {
            return Cow::Owned(Vec::new());
        };
        if stride == 0 {
            return Cow::Owned(Vec::new());
        }
        if stride == row_bytes {
            if data.len() < total_needed {
                return Cow::Owned(Vec::new());
            }
            return Cow::Borrowed(&data[..total_needed]);
        }
        let last_row_start = match height.checked_sub(1).and_then(|r| r.checked_mul(stride)) {
            Some(o) => o,
            None => return Cow::Owned(Vec::new()),
        };
        let Some(end) = last_row_start.checked_add(row_bytes) else {
            return Cow::Owned(Vec::new());
        };
        if end > data.len() {
            return Cow::Owned(Vec::new());
        }
        // 行ごとにコピーする。フレームはデバイス由来で行数を信用しない領域確保を避ける。
        let mut result = Vec::new();
        for row in 0..height {
            let start = row * stride;
            let Some(row_end) = start.checked_add(row_bytes) else {
                return Cow::Owned(Vec::new());
            };
            if row_end > data.len() {
                return Cow::Owned(Vec::new());
            }
            result.extend_from_slice(&data[start..row_end]);
        }
        Cow::Owned(result)
    }

    /// デバイス列挙結果を表示する共通処理
    fn print_device_list(list: &VideoDeviceList) {
        println!("=== Video Devices ===");
        if list.is_empty() {
            println!("  No video device found");
            return;
        }
        for device in list {
            let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
            let id = device.unique_id().unwrap_or_else(|_| "Unknown".to_string());
            println!("  {name}");
            println!("    ID: {id}");
            for fmt in device.formats() {
                println!(
                    "    {}x{} @ {:.0}-{:.0} fps ({})",
                    fmt.width,
                    fmt.height,
                    fmt.min_fps,
                    fmt.max_fps,
                    fmt.pixel_format.name()
                );
            }
        }
    }

    /// VideoFrame の PixelFormat に応じて適切な enqueue メソッドを呼び出す。
    fn enqueue_video_frame(
        player: &VideoPlayer,
        frame: &VideoFrame<'_>,
    ) -> raw_player::error::Result<()> {
        if frame.width <= 0 || frame.height <= 0 {
            static LOG: Once = Once::new();
            LOG.call_once(|| {
                eprintln!(
                    "enqueue_video_frame: invalid frame dimensions (width/height must be positive)"
                );
            });
            return Ok(());
        }

        let w = frame.width as usize;
        let h = frame.height as usize;
        let stride = frame.stride as usize;
        let stride_uv = frame.stride_uv as usize;
        let pts_us = frame.timestamp_us;

        match frame.pixel_format {
            PixelFormat::Nv12 => {
                let y = strip_stride(frame.data, w, h, stride);
                let Some(uv_data) = frame.uv_data else {
                    return Ok(());
                };
                let uv = strip_stride(uv_data, w, h.div_ceil(2), stride_uv);
                player.enqueue_video_nv12(&y, &uv, frame.width, frame.height, pts_us)?;
            }
            PixelFormat::I420 => {
                let y = strip_stride(frame.data, w, h, stride);
                let Some(uv_data) = frame.uv_data else {
                    return Ok(());
                };
                let uv_w = w.div_ceil(2);
                let uv_h = h.div_ceil(2);
                let Some(u_plane_size) = stride_uv.checked_mul(uv_h) else {
                    return Ok(());
                };
                let Some(uv_total) = u_plane_size.checked_mul(2) else {
                    return Ok(());
                };
                if uv_data.len() < uv_total {
                    static LOG_I420: Once = Once::new();
                    LOG_I420.call_once(|| {
                        eprintln!(
                            "enqueue_video_frame: I420 uv_data slice too short for U and V planes"
                        );
                    });
                    return Ok(());
                }
                let u = strip_stride(&uv_data[..u_plane_size], uv_w, uv_h, stride_uv);
                let v = strip_stride(&uv_data[u_plane_size..uv_total], uv_w, uv_h, stride_uv);
                player.enqueue_video_i420(&y, &u, &v, frame.width, frame.height, pts_us)?;
            }
            PixelFormat::Yuy2 => {
                let data = strip_stride(frame.data, w * 2, h, stride);
                player.enqueue_video_yuy2(&data, frame.width, frame.height, pts_us)?;
            }
            PixelFormat::Mjpeg => {
                static WARN: Once = Once::new();
                WARN.call_once(|| {
                    eprintln!("camera_preview: MJPEG format is not supported in this sample");
                });
                return Ok(());
            }
            PixelFormat::Unknown(_) => {
                static WARN: Once = Once::new();
                WARN.call_once(|| {
                    eprintln!(
                        "camera_preview: unsupported pixel format: {}",
                        frame.pixel_format.name()
                    );
                });
            }
        }
        Ok(())
    }

    pub fn main() -> noargs::Result<()> {
        let args = parse_args()?;

        if args.list_devices {
            let device_list = VideoDeviceList::enumerate();
            print_device_list(&device_list.expect("failed to enumerate devices"));
            return Ok(());
        }

        // キャプチャスレッドからメインスレッドへフレームを送るチャネル。
        // 無制限 mpsc はメモリ増大しうるため、上限付き sync_channel と try_send でブロックしない背圧とする。
        let (tx, rx) = sync_channel::<VideoFrameOwned>(4);

        // VideoCapture を作成
        let video_config = VideoCaptureConfig {
            device_id: args.video_device_id,
            width: args.width,
            height: args.height,
            fps: args.fps,
            pixel_format: None,
        };
        let callback = move |frame: VideoFrame<'_>| {
            if tx.try_send(frame.to_owned()).is_err() {
                static DROP_LOG: Once = Once::new();
                DROP_LOG.call_once(|| {
                    eprintln!(
                        "camera_preview: dropped frame (channel full or receiver disconnected); further drops are silent"
                    );
                });
            }
        };
        let mut video_capture =
            VideoCapture::new(video_config, callback).expect("failed to create VideoCapture");

        // VideoPlayer を作成
        let title = format!(
            "Camera Preview ({}x{} @ {} fps)",
            args.width, args.height, args.fps
        );
        let player = VideoPlayer::new(args.width, args.height, &title)
            .expect("failed to create VideoPlayer");

        println!("=== Camera Preview ===");
        println!("Resolution: {}x{}", args.width, args.height);
        println!("FPS: {}", args.fps);
        println!("GPU Renderer: {}", player.renderer_name());
        if let Some(duration) = args.duration {
            println!("Duration: {duration} sec");
        }

        // ESC キーで終了
        player.set_key_callback(Some(|keycode: u32| -> bool { keycode != KEYCODE_ESCAPE }));

        // キャプチャ開始
        video_capture
            .start()
            .expect("failed to start video capture");

        // 再生開始
        player.play().expect("failed to start playback");

        println!();
        println!("Press ESC to exit, S to toggle the statistics overlay");
        println!();

        let start = Instant::now();

        // メインループ
        loop {
            // キャプチャスレッドからのフレームを受信して enqueue
            while let Ok(owned) = rx.try_recv() {
                let frame = owned.as_frame();
                if let Err(e) = enqueue_video_frame(&player, &frame) {
                    eprintln!("failed to enqueue video frame: {e}");
                }
            }

            // SDL イベント処理とフレームレンダリング
            match player.poll_events() {
                Ok(true) => {}
                _ => break,
            }

            if let Some(duration) = args.duration
                && start.elapsed().as_secs_f64() >= duration
            {
                break;
            }
        }

        // 停止
        video_capture.stop();

        // 統計表示
        let stats = player.stats();
        let elapsed = start.elapsed().as_secs_f64();

        println!();
        println!("=== Statistics ===");
        println!("Elapsed: {elapsed:.2} sec");
        println!(
            "Video frames: enqueue={}, render={}, drop={}, repeat={}",
            stats.total_frames_enqueued,
            stats.total_frames_rendered,
            stats.dropped_frames,
            stats.repeated_frames,
        );
        println!("FPS: {:.1}", stats.current_fps);

        // SDL リソースをすべて解放してから SDL を終了する
        drop(player);
        // SAFETY: player を drop 済みなので SDL リソースは解放されている
        unsafe { raw_player::quit() };
        println!("Done");
        Ok(())
    }
}

#[cfg(any(enable_avf, enable_v4l2, enable_pipewire, enable_mf))]
fn main() -> noargs::Result<()> {
    imp::main()
}

#[cfg(not(any(enable_avf, enable_v4l2, enable_pipewire, enable_mf)))]
fn main() {
    eprintln!("No video backend feature is enabled. Enable one of avf, v4l2, pipewire, or mf.");
    std::process::exit(1);
}
