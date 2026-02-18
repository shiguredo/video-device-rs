use shiguredo_libcamera::{CameraManager, ConfigStatus, StreamRole};

fn main() {
    let manager = match CameraManager::new() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("カメラマネージャーの作成に失敗しました: {e}");
            std::process::exit(1);
        }
    };

    let camera_count = manager.cameras_count();

    struct CameraInfo {
        id: String,
        streams: Vec<StreamInfo>,
    }

    struct StreamInfo {
        pixel_format: String,
        width: u32,
        height: u32,
        buffer_count: u32,
    }

    let mut cameras = Vec::new();

    for i in 0..camera_count {
        let camera = match manager.get_camera(i) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("カメラ {i} の取得に失敗しました: {e}");
                continue;
            }
        };

        let id = camera.id().to_string();

        if let Err(e) = camera.acquire() {
            eprintln!("カメラ {id} の acquire に失敗しました: {e}");
            continue;
        }

        let mut streams = Vec::new();

        if let Ok(mut config) = camera.generate_configuration(&[StreamRole::Viewfinder]) {
            if let Ok(status) = config.validate() {
                if status != ConfigStatus::Invalid {
                    for j in 0..config.size() as u32 {
                        if let Ok(sc) = config.at(j) {
                            let pixel_format = sc.pixel_format().to_string();
                            let size = sc.size();
                            streams.push(StreamInfo {
                                pixel_format,
                                width: size.width,
                                height: size.height,
                                buffer_count: sc.buffer_count(),
                            });
                        }
                    }
                }
            }
        }

        let _ = camera.release();

        cameras.push(CameraInfo { id, streams });
    }

    let output = nojson::json(|f| {
        f.set_indent_size(2);
        f.set_spacing(true);
        f.object(|f| {
            f.member("camera_count", camera_count)?;
            f.member(
                "cameras",
                nojson::array(|f| {
                    for camera in &cameras {
                        f.element(nojson::object(|f| {
                            f.member("id", camera.id.as_str())?;
                            f.member(
                                "streams",
                                nojson::array(|f| {
                                    for stream in &camera.streams {
                                        f.element(nojson::object(|f| {
                                            f.member("pixel_format", stream.pixel_format.as_str())?;
                                            f.member("width", stream.width)?;
                                            f.member("height", stream.height)?;
                                            f.member("buffer_count", stream.buffer_count)
                                        }))?;
                                    }
                                    Ok(())
                                }),
                            )
                        }))?;
                    }
                    Ok(())
                }),
            )
        })
    });

    println!("{output}");
}
