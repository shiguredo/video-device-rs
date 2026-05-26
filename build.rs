use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let src_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    match target_os.as_str() {
        "macos" => {
            build_macos(&src_dir);
            generate_bindings(&src_dir.join("video_avf.h"), "bindings_avf.rs", &out_dir);
        }
        "linux" => {
            let has_v4l2 = env::var("CARGO_FEATURE_V4L2").is_ok();
            let has_pipewire = env::var("CARGO_FEATURE_PIPEWIRE").is_ok();

            if has_v4l2 {
                build_linux_v4l2(&src_dir);
                generate_bindings(&src_dir.join("video_v4l2.h"), "bindings_v4l2.rs", &out_dir);
            }
            if has_pipewire {
                build_linux_pipewire(&src_dir);
                generate_bindings(
                    &src_dir.join("video_pipewire.h"),
                    "bindings_pipewire.rs",
                    &out_dir,
                );
            }
        }
        "windows" => {
            // windows-rs を使用するため C/C++ コンパイル不要
            // bindgen も不要
        }
        _ => panic!("Unsupported target OS: {}", target_os),
    }
}

fn build_macos(src_dir: &Path) {
    println!("cargo::rerun-if-changed=src/video_avf.m");
    println!("cargo::rerun-if-changed=src/video_avf.h");
    println!("cargo::rerun-if-changed=src/video_common.h");

    // Objective-C ファイルをコンパイル
    cc::Build::new()
        .file(src_dir.join("video_avf.m"))
        .flag("-fobjc-arc")
        .compile("video_avf");

    // macOS フレームワークをリンク
    println!("cargo::rustc-link-lib=framework=AVFoundation");
    println!("cargo::rustc-link-lib=framework=CoreFoundation");
    println!("cargo::rustc-link-lib=framework=CoreMedia");
    println!("cargo::rustc-link-lib=framework=CoreVideo");
    println!("cargo::rustc-link-lib=framework=Foundation");
}

fn build_linux_v4l2(src_dir: &Path) {
    println!("cargo::rerun-if-changed=src/video_v4l2.c");
    println!("cargo::rerun-if-changed=src/video_v4l2.h");
    println!("cargo::rerun-if-changed=src/video_common.h");

    // V4L2 C ファイルをコンパイル
    cc::Build::new()
        .file(src_dir.join("video_v4l2.c"))
        .compile("video_v4l2");

    // pthread をリンク
    println!("cargo::rustc-link-lib=pthread");
}

fn build_linux_pipewire(src_dir: &Path) {
    println!("cargo::rerun-if-changed=src/video_pipewire.c");
    println!("cargo::rerun-if-changed=src/video_pipewire.h");
    println!("cargo::rerun-if-changed=src/video_common.h");

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

fn generate_bindings(header: &Path, out_file: &str, out_dir: &Path) {
    let bindings = bindgen::Builder::default()
        .header(header.to_str().unwrap())
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
