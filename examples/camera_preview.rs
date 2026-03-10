//! カメラ映像をキャプチャして raw-player でプレビュー表示するサンプル
//!
//! shiguredo_video_device でカメラ映像を取得し、
//! raw_player::VideoPlayer でリアルタイム表示する。
//!
//! ```
//! cargo run --example camera_preview -- --list-devices
//! cargo run --example camera_preview
//! cargo run --example camera_preview -- --resolution 1080p --fps 60
//! ```

use std::borrow::Cow;
use std::sync::mpsc;
use std::time::Instant;

use raw_player::{KEYCODE_ESCAPE, VideoPlayer};
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

impl Default for Args {
    fn default() -> Self {
        Self {
            list_devices: false,
            video_device_id: None,
            width: 1280,
            height: 720,
            fps: 30,
            duration: None,
        }
    }
}

fn print_usage() {
    eprintln!(
        "Usage: camera_preview [OPTIONS]

Options:
  --list-devices              デバイス一覧を表示して終了
  --video-device-id <id>      映像デバイス ID
  --resolution <value>        解像度 (720p, 1080p, 4k, WxH) [default: 720p]
  --fps <n>                   フレームレート [default: 30]
  --duration <sec>            再生時間 (秒)
  -h, --help                  ヘルプを表示"
    );
}

fn parse_resolution(s: &str) -> Option<(i32, i32)> {
    match s.to_lowercase().as_str() {
        "4k" | "2160p" => Some((3840, 2160)),
        "1080p" => Some((1920, 1080)),
        "720p" => Some((1280, 720)),
        "540p" => Some((960, 540)),
        _ => {
            let parts: Vec<&str> = s.split('x').collect();
            if parts.len() == 2 {
                let w = parts[0].parse().ok()?;
                let h = parts[1].parse().ok()?;
                Some((w, h))
            } else {
                None
            }
        }
    }
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    let mut result = Args::default();

    while i < args.len() {
        match args[i].as_str() {
            "--list-devices" => result.list_devices = true,
            "--video-device-id" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("エラー: --video-device-id に値が必要です");
                    std::process::exit(1);
                }
                result.video_device_id = Some(args[i].clone());
            }
            "--resolution" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("エラー: --resolution に値が必要です");
                    std::process::exit(1);
                }
                if let Some((w, h)) = parse_resolution(&args[i]) {
                    result.width = w;
                    result.height = h;
                } else {
                    eprintln!("エラー: 不正な解像度: {}", args[i]);
                    std::process::exit(1);
                }
            }
            "--fps" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("エラー: --fps に値が必要です");
                    std::process::exit(1);
                }
                result.fps = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("エラー: 不正な fps: {}", args[i]);
                    std::process::exit(1);
                });
            }
            "--duration" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("エラー: --duration に値が必要です");
                    std::process::exit(1);
                }
                result.duration = Some(args[i].parse().unwrap_or_else(|_| {
                    eprintln!("エラー: 不正な duration: {}", args[i]);
                    std::process::exit(1);
                }));
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => {
                eprintln!("エラー: 不明な引数: {other}");
                print_usage();
                std::process::exit(1);
            }
        }
        i += 1;
    }
    result
}

/// stride を除去してピクセルデータのみを取り出す。
/// stride == row_bytes の場合はゼロコピーで借用を返す。
fn strip_stride<'a>(
    data: &'a [u8],
    row_bytes: usize,
    height: usize,
    stride: usize,
) -> Cow<'a, [u8]> {
    if stride == row_bytes {
        Cow::Borrowed(&data[..row_bytes * height])
    } else {
        let mut result = Vec::with_capacity(row_bytes * height);
        for row in 0..height {
            let start = row * stride;
            result.extend_from_slice(&data[start..start + row_bytes]);
        }
        Cow::Owned(result)
    }
}

fn list_devices() {
    println!("=== 映像デバイス一覧 ===");
    match VideoDeviceList::enumerate() {
        Ok(devices) => {
            if devices.is_empty() {
                println!("  映像デバイスが見つかりません");
            } else {
                for device in devices.devices() {
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
        }
        Err(e) => eprintln!("  映像デバイスの列挙に失敗: {e:?}"),
    }
}

/// VideoFrame の PixelFormat に応じて適切な enqueue メソッドを呼び出す。
fn enqueue_video_frame(player: &VideoPlayer, frame: &VideoFrame<'_>) -> raw_player::Result<()> {
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
            let uv = strip_stride(uv_data, w, h / 2, stride_uv);
            player.enqueue_video_nv12(&y, &uv, frame.width, frame.height, pts_us)?;
        }
        PixelFormat::I420 => {
            let y = strip_stride(frame.data, w, h, stride);
            let Some(uv_data) = frame.uv_data else {
                return Ok(());
            };
            let uv_w = w / 2;
            let uv_h = h / 2;
            let u_plane_size = stride_uv * uv_h;
            let u = strip_stride(&uv_data[..u_plane_size], uv_w, uv_h, stride_uv);
            let v = strip_stride(&uv_data[u_plane_size..], uv_w, uv_h, stride_uv);
            player.enqueue_video_i420(&y, &u, &v, frame.width, frame.height, pts_us)?;
        }
        PixelFormat::Yuy2 => {
            let data = strip_stride(frame.data, w * 2, h, stride);
            player.enqueue_video_yuy2(&data, frame.width, frame.height, pts_us)?;
        }
        PixelFormat::Unknown(_) => {
            use std::sync::Once;
            static WARN: Once = Once::new();
            WARN.call_once(|| {
                eprintln!(
                    "警告: 未対応のピクセルフォーマット: {}",
                    frame.pixel_format.name()
                );
            });
        }
    }
    Ok(())
}

fn main() {
    let args = parse_args();

    if args.list_devices {
        list_devices();
        return;
    }

    // キャプチャスレッドからメインスレッドへフレームを送るチャネル
    let (tx, rx) = mpsc::channel::<VideoFrameOwned>();

    // VideoCapture を作成
    let video_config = VideoCaptureConfig {
        device_id: args.video_device_id,
        width: args.width,
        height: args.height,
        fps: args.fps,
        pixel_format: None,
    };
    let mut video_capture = VideoCapture::new(video_config, move |frame: VideoFrame<'_>| {
        let _ = tx.send(frame.to_owned());
    })
    .expect("VideoCapture の作成に失敗しました");

    // VideoPlayer を作成
    let title = format!(
        "Camera Preview ({}x{} @ {} fps)",
        args.width, args.height, args.fps
    );
    let player = VideoPlayer::new(args.width, args.height, &title)
        .expect("VideoPlayer の作成に失敗しました");

    println!("=== Camera Preview ===");
    println!("解像度: {}x{}", args.width, args.height);
    println!("FPS: {}", args.fps);
    println!("GPU Renderer: {}", player.renderer_name());
    if let Some(duration) = args.duration {
        println!("再生時間: {duration} 秒");
    }

    // ESC キーで終了
    player.set_key_callback(Some(|keycode: u32| -> bool { keycode != KEYCODE_ESCAPE }));

    // キャプチャ開始
    video_capture
        .start()
        .expect("映像キャプチャの開始に失敗しました");

    // 再生開始
    player.play().expect("再生の開始に失敗しました");

    println!();
    println!("ESC キーで終了, S キーで統計オーバーレイ切替");
    println!();

    let start = Instant::now();

    // メインループ
    loop {
        // キャプチャスレッドからのフレームを受信して enqueue
        while let Ok(owned) = rx.try_recv() {
            let frame = owned.as_frame();
            if let Err(e) = enqueue_video_frame(&player, &frame) {
                eprintln!("映像フレームの enqueue に失敗: {e}");
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
    println!("=== 統計 ===");
    println!("時間: {elapsed:.2} 秒");
    println!(
        "映像フレーム: enqueue={}, render={}, drop={}, repeat={}",
        stats.total_frames_enqueued,
        stats.total_frames_rendered,
        stats.dropped_frames,
        stats.repeated_frames,
    );
    println!("FPS: {:.1}", stats.current_fps);

    // SDL リソースをすべて解放してから SDL を終了する
    drop(player);
    raw_player::quit();
    println!("完了");
}
