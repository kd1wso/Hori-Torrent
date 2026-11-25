//! Hori Torrent core: a thin, Android-friendly wrapper around the librqbit
//! BitTorrent engine. The same engine is exposed to Kotlin through JNI
//! (see `jni_bridge.rs`) and to a desktop CLI (see `bin/hori.rs`).

pub mod engine;

#[cfg(target_os = "android")]
mod jni_bridge;

pub use engine::Engine;

/// Install the ring crypto provider as the process-wide rustls default.
/// Must run before any TLS client is built (both the Android JNI init and
/// the desktop CLI call this at startup).
pub fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}
