use std::sync::{Arc, Mutex};

use shiguredo_libcamera::{
    CameraManager, ConfigStatus, FrameBufferAllocator, FrameStatus, StreamRole,
};

const CAPTURE_FRAME_COUNT: usize = 5;

fn main() {
    let manager = match CameraManager::new() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("カメラマネージャーの作成に失敗しました: {e}");
            std::process::exit(1);
        }
    };

    if manager.cameras_count() == 0 {
        eprintln!("カメラが見つかりません");
        std::process::exit(1);
    }

    let mut camera = match manager.get_camera(0) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("カメラの取得に失敗しました: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = camera.acquire() {
        eprintln!("カメラの acquire に失敗しました: {e}");
        std::process::exit(1);
    }

    let mut config = match camera.generate_configuration(&[StreamRole::VideoRecording]) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("設定の生成に失敗しました: {e}");
            std::process::exit(1);
        }
    };

    match config.validate() {
        Ok(ConfigStatus::Invalid) => {
            eprintln!("設定が無効です");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("設定のバリデーションに失敗しました: {e}");
            std::process::exit(1);
        }
        Ok(_) => {}
    }

    if let Err(e) = camera.configure(&mut config) {
        eprintln!("カメラの設定に失敗しました: {e}");
        std::process::exit(1);
    }

    let stream = match config.at(0) {
        Ok(sc) => match sc.stream() {
            Some(s) => s,
            None => {
                eprintln!("ストリームの取得に失敗しました");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("ストリーム設定の取得に失敗しました: {e}");
            std::process::exit(1);
        }
    };

    let allocator = FrameBufferAllocator::new(&camera);
    let buffer_count = match allocator.allocate(&stream) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("バッファの割り当てに失敗しました: {e}");
            std::process::exit(1);
        }
    };

    struct FrameInfo {
        sequence: u32,
        timestamp: u64,
        status: String,
    }

    let frames: Arc<Mutex<Vec<FrameInfo>>> = Arc::new(Mutex::new(Vec::new()));

    let frames_cb = Arc::clone(&frames);
    let stream_cb = stream.clone();
    camera.on_request_completed(move |completed| {
        if let Some(buffer) = completed.find_buffer(&stream_cb) {
            let meta = buffer.metadata();
            let status = match meta.status {
                FrameStatus::Success => "success",
                FrameStatus::Error => "error",
                FrameStatus::Cancelled => "cancelled",
                FrameStatus::Startup => "startup",
            };
            let mut guard = frames_cb.lock().unwrap();
            guard.push(FrameInfo {
                sequence: meta.sequence,
                timestamp: meta.timestamp,
                status: status.to_string(),
            });
        }
    });

    if let Err(e) = camera.start() {
        eprintln!("カメラの開始に失敗しました: {e}");
        std::process::exit(1);
    }

    let mut requests = Vec::new();
    for i in 0..buffer_count {
        let buffer = match allocator.get_buffer(&stream, i) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("バッファ {i} の取得に失敗しました: {e}");
                std::process::exit(1);
            }
        };
        let request = match camera.create_request(i as u64) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("リクエストの作成に失敗しました: {e}");
                std::process::exit(1);
            }
        };
        if let Err(e) = request.add_buffer(&stream, &buffer) {
            eprintln!("バッファの追加に失敗しました: {e}");
            std::process::exit(1);
        }
        requests.push(request);
    }

    for request in &requests {
        if let Err(e) = camera.queue_request(request) {
            eprintln!("リクエストのキューイングに失敗しました: {e}");
            std::process::exit(1);
        }
    }

    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let guard = frames.lock().unwrap();
        if guard.len() >= CAPTURE_FRAME_COUNT {
            break;
        }
    }

    if let Err(e) = camera.stop() {
        eprintln!("カメラの停止に失敗しました: {e}");
        std::process::exit(1);
    }

    let _ = camera.release();

    let guard = frames.lock().unwrap();
    let output = nojson::json(|f| {
        f.set_indent_size(2);
        f.set_spacing(true);
        f.object(|f| {
            f.member("camera_id", camera.id())?;
            f.member("requested_frames", CAPTURE_FRAME_COUNT)?;
            f.member("captured_frames", guard.len())?;
            f.member(
                "frames",
                nojson::array(|f| {
                    for frame in guard.iter() {
                        f.element(nojson::object(|f| {
                            f.member("sequence", frame.sequence)?;
                            f.member("timestamp", frame.timestamp)?;
                            f.member("status", frame.status.as_str())
                        }))?;
                    }
                    Ok(())
                }),
            )
        })
    });

    println!("{output}");
}
