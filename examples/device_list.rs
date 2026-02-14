use shiguredo_video_device::VideoDeviceList;

fn main() {
    let device_list = match VideoDeviceList::enumerate() {
        Ok(list) => list,
        Err(e) => {
            eprintln!("デバイスの列挙に失敗しました: {e}");
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
