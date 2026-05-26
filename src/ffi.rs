#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

#[cfg(all(target_os = "linux", feature = "v4l2"))]
pub mod v4l2 {
    include!(concat!(env!("OUT_DIR"), "/bindings_v4l2.rs"));
}

#[cfg(all(target_os = "linux", feature = "pipewire"))]
pub mod pipewire {
    include!(concat!(env!("OUT_DIR"), "/bindings_pipewire.rs"));
}

#[cfg(target_os = "macos")]
pub mod avf {
    include!(concat!(env!("OUT_DIR"), "/bindings_avf.rs"));
}
