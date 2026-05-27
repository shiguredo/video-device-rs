#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

#[cfg(target_os = "macos")]
include!(concat!(env!("OUT_DIR"), "/bindings_macos.rs"));

#[cfg(target_os = "linux")]
include!(concat!(env!("OUT_DIR"), "/bindings_linux.rs"));
