use std::sync::{Arc, Mutex};

use shiguredo_libcamera::{CameraManager, ConfigStatus, FrameBufferAllocator, StreamRole, core};

const CAPTURE_FRAME_COUNT: usize = 3;

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

    let mut config = match camera.generate_configuration(&[StreamRole::Viewfinder]) {
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

    struct FrameMetadata {
        sequence: u32,
        exposure_time: Option<i32>,
        analogue_gain: Option<f32>,
        colour_temperature: Option<i32>,
        brightness: Option<f32>,
        sensor_timestamp: Option<i64>,
    }

    let frames: Arc<Mutex<Vec<FrameMetadata>>> = Arc::new(Mutex::new(Vec::new()));

    let frames_cb = Arc::clone(&frames);
    camera.on_request_completed(move |completed| {
        let meta = completed.metadata();
        let mut guard = frames_cb.lock().unwrap();
        guard.push(FrameMetadata {
            sequence: completed.sequence(),
            exposure_time: meta.get_i32(&core::EXPOSURE_TIME).ok(),
            analogue_gain: meta.get_f32(&core::ANALOGUE_GAIN).ok(),
            colour_temperature: meta.get_i32(&core::COLOUR_TEMPERATURE).ok(),
            brightness: meta.get_f32(&core::BRIGHTNESS).ok(),
            sensor_timestamp: meta.get_i64(&core::SENSOR_TIMESTAMP).ok(),
        });
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

        let mut controls = request.controls();
        controls.set_f32(&core::BRIGHTNESS, 0.2);
        controls.set_f32(&core::CONTRAST, 1.5);
        controls.set_f32(&core::SATURATION, 1.2);

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
            f.member(
                "controls_set",
                nojson::object(|f| {
                    f.member("brightness", 0.2f32)?;
                    f.member("contrast", 1.5f32)?;
                    f.member("saturation", 1.2f32)
                }),
            )?;
            f.member("captured_frames", guard.len())?;
            f.member(
                "frames",
                nojson::array(|f| {
                    for frame in guard.iter() {
                        f.element(nojson::object(|f| {
                            f.member("sequence", frame.sequence)?;
                            if let Some(v) = frame.exposure_time {
                                f.member("exposure_time", v)?;
                            }
                            if let Some(v) = frame.analogue_gain {
                                f.member("analogue_gain", v)?;
                            }
                            if let Some(v) = frame.colour_temperature {
                                f.member("colour_temperature", v)?;
                            }
                            if let Some(v) = frame.brightness {
                                f.member("brightness", v)?;
                            }
                            if let Some(v) = frame.sensor_timestamp {
                                f.member("sensor_timestamp", v)?;
                            }
                            Ok(())
                        }))?;
                    }
                    Ok(())
                }),
            )
        })
    });

    println!("{output}");
}
