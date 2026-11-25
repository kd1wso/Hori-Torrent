//! Hori Torrent core: a thin, Android-friendly wrapper around the librqbit
//! BitTorrent engine. The same engine is exposed to Kotlin through JNI
//! (see `jni_bridge.rs`) and to a desktop CLI (see `bin/hori.rs`).

pub mod engine;

#[cfg(target_os = "android")]
mod jni_bridge;

pub use engine::Engine;
