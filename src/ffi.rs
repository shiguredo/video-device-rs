#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

#[cfg(enable_avf)]
include!(concat!(env!("OUT_DIR"), "/bindings_macos.rs"));

#[cfg(any(enable_v4l2, enable_pipewire))]
include!(concat!(env!("OUT_DIR"), "/bindings_linux.rs"));
