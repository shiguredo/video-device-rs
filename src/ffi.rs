//! C / Objective-C バックエンドの FFI バインディング。
//!
//! `build.rs` が `bindgen` で生成したバインディングを `include!` で取り込む。
//! バックエンドごとに読み込むファイルを分けている。
//!
//! `build.rs` は有効な feature のヘッダだけを `bindgen` に渡すため、
//! 生成物に含まれる関数は必ずいずれかの OPS テーブルから参照される。

#[cfg(enable_avf)]
include!(concat!(env!("OUT_DIR"), "/bindings_macos.rs"));

#[cfg(any(enable_v4l2, enable_pipewire))]
include!(concat!(env!("OUT_DIR"), "/bindings_linux.rs"));
