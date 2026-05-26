use std::collections::{BTreeSet, HashMap};

use shiguredo_video_device::{PixelFormat, VideoDevice, VideoDeviceList};

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

    let mut pixel_format_counts: HashMap<PixelFormat, usize> = HashMap::new();
    let mut resolutions: BTreeSet<(i32, i32)> = BTreeSet::new();
    let mut total_format_count: usize = 0;
    let mut fps_min = f32::MAX;
    let mut fps_max = f32::MIN;

    for device in device_list.devices() {
        let formats = device.formats();
        for format in &formats {
            total_format_count += 1;
            *pixel_format_counts.entry(format.pixel_format).or_insert(0) += 1;
            resolutions.insert((format.width, format.height));
            if format.min_fps < fps_min {
                fps_min = format.min_fps;
            }
            if format.max_fps > fps_max {
                fps_max = format.max_fps;
            }
        }
    }

    if total_format_count == 0 {
        fps_min = 0.0;
        fps_max = 0.0;
    }

    let mut sorted_pixel_formats: Vec<_> = pixel_format_counts.into_iter().collect();
    sorted_pixel_formats.sort_by_key(|(fmt, _)| fmt.name());

    let output = nojson::json(|f| {
        f.set_indent_size(2);
        f.set_spacing(true);
        f.object(|f| {
            f.member(
                "devices",
                nojson::array(|f| {
                    for device in device_list.devices() {
                        let name = device.name().unwrap_or_default();
                        let unique_id = device.unique_id().unwrap_or_default();
                        let formats = device.formats();
                        let format_count = formats.len();
                        f.element(nojson::object(|f| {
                            f.member("name", name.as_str())?;
                            f.member("unique_id", unique_id.as_str())?;
                            f.member("format_count", format_count)?;
                            f.member(
                                "formats",
                                nojson::array(|f| {
                                    for fmt in &formats {
                                        f.element(nojson::object(|f| {
                                            f.member("width", fmt.width)?;
                                            f.member("height", fmt.height)?;
                                            f.member("pixel_format", fmt.pixel_format.name())?;
                                            f.member("min_fps", fmt.min_fps)?;
                                            f.member("max_fps", fmt.max_fps)
                                        }))?;
                                    }
                                    Ok(())
                                }),
                            )
                        }))?;
                    }
                    Ok(())
                }),
            )?;
            f.member(
                "statistics",
                nojson::object(|f| {
                    f.member("device_count", device_list.len())?;
                    f.member("total_format_count", total_format_count)?;
                    f.member(
                        "pixel_formats",
                        nojson::object(|f| {
                            for (format, count) in &sorted_pixel_formats {
                                f.member(format.name(), *count)?;
                            }
                            Ok(())
                        }),
                    )?;
                    f.member(
                        "resolutions",
                        nojson::array(|f| {
                            for (w, h) in &resolutions {
                                f.element(format!("{w}x{h}"))?;
                            }
                            Ok(())
                        }),
                    )?;
                    f.member("fps_min", fps_min)?;
                    f.member("fps_max", fps_max)
                }),
            )
        })
    });

    println!("{output}");
}
