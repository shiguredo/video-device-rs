use std::env;
use std::path::{Path, PathBuf};

use bindgen::Builder;

fn main() {
    // 対応するプラットフォームだった場合だけ各 feature を有効にする。
    // これによって以下のように書けるようになる。
    //
    // #[cfg(all(target_os = "linux", feature = "v4l2"))]
    // ↓
    // #[cfg(enable_v4l2)]
    println!("cargo::rustc-check-cfg=cfg(enable_avf)");
    println!("cargo::rustc-check-cfg=cfg(enable_v4l2)");
    println!("cargo::rustc-check-cfg=cfg(enable_pipewire)");
    println!("cargo::rustc-check-cfg=cfg(enable_mf)");
    println!("cargo::rustc-check-cfg=cfg(enable_mjpeg)");
    println!("cargo::rustc-check-cfg=cfg(enable_default_avf)");
    println!("cargo::rustc-check-cfg=cfg(enable_default_v4l2)");
    println!("cargo::rustc-check-cfg=cfg(enable_default_pipewire)");
    println!("cargo::rustc-check-cfg=cfg(enable_default_mf)");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let mut enable_default_count = 0;
    match env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
        "macos" => {
            if env::var("CARGO_FEATURE_AVF").is_ok() {
                println!("cargo::rustc-cfg=enable_avf");
            }
            if env::var("CARGO_FEATURE_DEFAULT_AVF").is_ok() {
                println!("cargo::rustc-cfg=enable_default_avf");
                enable_default_count += 1;
            }
        }
        "linux" => {
            if env::var("CARGO_FEATURE_V4L2").is_ok() {
                println!("cargo::rustc-cfg=enable_v4l2");
            }
            if env::var("CARGO_FEATURE_MJPEG").is_ok() {
                println!("cargo::rustc-cfg=enable_mjpeg");
            }
            if env::var("CARGO_FEATURE_PIPEWIRE").is_ok() {
                println!("cargo::rustc-cfg=enable_pipewire");
            }
            if env::var("CARGO_FEATURE_DEFAULT_V4L2").is_ok() {
                println!("cargo::rustc-cfg=enable_default_v4l2");
                enable_default_count += 1;
            }
            if env::var("CARGO_FEATURE_DEFAULT_PIPEWIRE").is_ok() {
                println!("cargo::rustc-cfg=enable_default_pipewire");
                enable_default_count += 1;
            }
        }
        "windows" => {
            if env::var("CARGO_FEATURE_MF").is_ok() {
                println!("cargo::rustc-cfg=enable_mf");
            }
            if env::var("CARGO_FEATURE_DEFAULT_MF").is_ok() {
                println!("cargo::rustc-cfg=enable_default_mf");
                enable_default_count += 1;
            }
        }
        _ => panic!("Unsupported target OS: {}", target_os),
    }

    if enable_default_count >= 2 {
        panic!("Multiple default backends selected. Enable exactly one default-* feature.");
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let src_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src");

    match target_os.as_str() {
        "macos" => {
            if env::var("CARGO_FEATURE_AVF").is_ok() {
                build_macos(&src_dir);
                let builder =
                    Builder::default().header(src_dir.join("video_avf.h").to_str().unwrap());
                generate_bindings(builder, "bindings_macos.rs", &out_dir);
            }
        }
        "linux" => {
            let has_v4l2 = env::var("CARGO_FEATURE_V4L2").is_ok();
            let has_pipewire = env::var("CARGO_FEATURE_PIPEWIRE").is_ok();

            if has_v4l2 {
                build_linux_v4l2(&src_dir);
            }
            if has_pipewire {
                build_linux_pipewire(&src_dir);
            }
            if has_v4l2 || has_pipewire {
                let mut builder = Builder::default();
                if has_v4l2 {
                    builder = builder.header(src_dir.join("video_v4l2.h").to_str().unwrap());
                }
                if has_pipewire {
                    builder = builder.header(src_dir.join("video_pipewire.h").to_str().unwrap());
                }
                generate_bindings(builder, "bindings_linux.rs", &out_dir);
            }
        }
        "windows" => {}
        _ => panic!("Unsupported target OS: {}", target_os),
    }
}

fn build_macos(src_dir: &Path) {
    println!("cargo::rerun-if-changed=src/video_avf.m");
    println!("cargo::rerun-if-changed=src/video_avf.h");
    println!("cargo::rerun-if-changed=src/video.h");

    cc::Build::new()
        .file(src_dir.join("video_avf.m"))
        .flag("-fobjc-arc")
        .compile("video_avf");

    println!("cargo::rustc-link-lib=framework=AVFoundation");
    println!("cargo::rustc-link-lib=framework=CoreFoundation");
    println!("cargo::rustc-link-lib=framework=CoreMedia");
    println!("cargo::rustc-link-lib=framework=CoreVideo");
    println!("cargo::rustc-link-lib=framework=Foundation");
}

fn build_linux_v4l2(src_dir: &Path) {
    println!("cargo::rerun-if-changed=src/video_v4l2.c");
    println!("cargo::rerun-if-changed=src/video_v4l2.h");
    println!("cargo::rerun-if-changed=src/video.h");

    let mut build = cc::Build::new();
    build.file(src_dir.join("video_v4l2.c"));
    if std::env::var("CARGO_FEATURE_MJPEG").is_ok() {
        build.define("SHIGUREDO_VIDEO_DEVICE_MJPEG", "1");
    }
    build.compile("video_v4l2");

    println!("cargo::rustc-link-lib=pthread");
}

fn build_linux_pipewire(src_dir: &Path) {
    println!("cargo::rerun-if-changed=src/video_pipewire.c");
    println!("cargo::rerun-if-changed=src/video_pipewire.h");
    println!("cargo::rerun-if-changed=src/video.h");

    let pipewire = pkg_config::Config::new()
        .probe("libpipewire-0.3")
        .expect("libpipewire-0.3 not found. Please install libpipewire-0.3-dev");

    let mut build = cc::Build::new();
    build.file(src_dir.join("video_pipewire.c"));

    for path in &pipewire.include_paths {
        build.include(path);
    }

    build.compile("video_pipewire");
}

fn generate_bindings(builder: Builder, out_file: &str, out_dir: &Path) {
    let bindings = builder
        .allowlist_function("video_.*")
        .allowlist_type("VideoDevice")
        .allowlist_type("VideoSession")
        .allowlist_type("VideoFormatEntry")
        .allowlist_type("FrameCallback")
        .allowlist_var("VIDEO_PIXEL_FORMAT_.*")
        .derive_default(true)
        .derive_debug(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Failed to generate bindings");

    bindings
        .write_to_file(out_dir.join(out_file))
        .expect("Failed to write bindings");
}
