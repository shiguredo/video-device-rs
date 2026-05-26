use shiguredo_video_device::{VideoDevice, VideoDeviceList};

#[cfg(target_os = "macos")]
use shiguredo_video_device::AvfVideoDeviceList;
#[cfg(target_os = "windows")]
use shiguredo_video_device::MfVideoDeviceList;
#[cfg(all(target_os = "linux", feature = "pipewire", not(feature = "v4l2")))]
use shiguredo_video_device::PipewireVideoDeviceList;
#[cfg(all(target_os = "linux", feature = "v4l2"))]
use shiguredo_video_device::V4l2VideoDeviceList;

fn main() {
    #[cfg(all(target_os = "linux", feature = "v4l2"))]
    let device_list = V4l2VideoDeviceList::enumerate();
    #[cfg(all(target_os = "linux", feature = "pipewire", not(feature = "v4l2")))]
    let device_list = PipewireVideoDeviceList::enumerate();
    #[cfg(target_os = "macos")]
    let device_list = AvfVideoDeviceList::enumerate();
    #[cfg(target_os = "windows")]
    let device_list = MfVideoDeviceList::enumerate();

    let device_list = match device_list {
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
                    for device in device_list.devices() {
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
