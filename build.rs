use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let src_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();

    println!("cargo::rerun-if-changed=src/video_c.h");

    match target_os.as_str() {
        "macos" => {
            build_macos(&src_dir);
            generate_bindings(&src_dir, &out_dir);
        }
        "linux" => {
            let has_v4l2 = env::var("CARGO_FEATURE_V4L2").is_ok();
            let has_pipewire = env::var("CARGO_FEATURE_PIPEWIRE").is_ok();

            if has_v4l2 && has_pipewire {
                panic!("features \"v4l2\" and \"pipewire\" are mutually exclusive");
            }

            if has_pipewire {
                build_linux_pipewire(&src_dir);
            } else {
                build_linux_v4l2(&src_dir);
            }

            generate_bindings(&src_dir, &out_dir);
        }
        "windows" => {
            // windows-rs を使用するため C/C++ コンパイル不要
            // bindgen も不要
        }
        _ => panic!("Unsupported target OS: {}", target_os),
    }
}

fn build_macos(src_dir: &Path) {
    println!("cargo::rerun-if-changed=src/video_c.m");

    // Objective-C ファイルをコンパイル
    cc::Build::new()
        .file(src_dir.join("video_c.m"))
        .flag("-fobjc-arc")
        .compile("video_c");

    // macOS フレームワークをリンク
    println!("cargo::rustc-link-lib=framework=AVFoundation");
    println!("cargo::rustc-link-lib=framework=CoreMedia");
    println!("cargo::rustc-link-lib=framework=CoreVideo");
    println!("cargo::rustc-link-lib=framework=Foundation");
}

fn build_linux_v4l2(src_dir: &Path) {
    println!("cargo::rerun-if-changed=src/video_v4l2.c");

    // V4L2 C ファイルをコンパイル
    cc::Build::new()
        .file(src_dir.join("video_v4l2.c"))
        .compile("video_c");

    // pthread をリンク
    println!("cargo::rustc-link-lib=pthread");
}

fn build_linux_pipewire(src_dir: &Path) {
    println!("cargo::rerun-if-changed=src/video_pipewire.c");

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

fn generate_bindings(src_dir: &Path, out_dir: &Path) {
    let bindings = bindgen::Builder::default()
        .header(src_dir.join("video_c.h").to_str().unwrap())
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

    let bindings_path = out_dir.join("bindings.rs");
    bindings
        .write_to_file(&bindings_path)
        .expect("Failed to write bindings");
}
