//! 全てのビデオデバイスを JSON で出力するサンプル。
//!
//! バックエンドが 1 つも有効でない場合はコンパイル対象から外す。

#[cfg(not(any(enable_avf, enable_v4l2, enable_pipewire, enable_mf)))]
fn main() {
    eprintln!("No video backend feature is enabled. Enable one of avf, v4l2, pipewire, or mf.");
    std::process::exit(1);
}

#[cfg(any(enable_avf, enable_v4l2, enable_pipewire, enable_mf))]
fn main() {
    use shiguredo_video_device::VideoDeviceList;

    let device_list = VideoDeviceList::enumerate();

    let device_list = match device_list {
        Ok(list) => list,
        Err(e) => {
            eprintln!("failed to enumerate devices: {e}");
            std::process::exit(1);
        }
    };

    let output = nojson::json(|f| {
        f.set_indent_size(2);
        f.set_spacing(true);
        f.object(|f| {
            f.member("device_count", device_list.len())?;
            f.member(
                "devices",
                nojson::array(|f| {
                    for device in &device_list {
                        let name = device.name().unwrap_or_default();
                        let unique_id = device.unique_id().unwrap_or_default();
                        let format_count = device.format_count();
                        f.element(nojson::object(|f| {
                            f.member("name", name.as_str())?;
                            f.member("unique_id", unique_id.as_str())?;
                            f.member("format_count", format_count)
                        }))?;
                    }
                    Ok(())
                }),
            )
        })
    });

    println!("{output}");
}
