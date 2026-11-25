//! JNI bridge: exposes the engine to `dev.hori.torrent.NativeBridge` (Kotlin).
//! Built on jni 0.22: each export upgrades `EnvUnowned` via `with_env`, and
//! errors (a simple std-error wrapper around a message) are thrown to Java
//! through the `ThrowRuntimeExAndDefault` policy.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::sys::{jboolean, jlong, JNI_TRUE};
use jni::EnvUnowned;

use crate::engine::Engine;

/// Minimal std::error::Error carrying a human-readable message for Java.
#[derive(Debug)]
struct BridgeError(String);

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for BridgeError {}

impl From<jni::errors::Error> for BridgeError {
    fn from(e: jni::errors::Error) -> Self {
        BridgeError(e.to_string())
    }
}

fn err(msg: impl std::fmt::Display) -> BridgeError {
    BridgeError(msg.to_string())
}

static ENGINE: OnceLock<Mutex<Option<Engine>>> = OnceLock::new();

fn engine_slot() -> &'static Mutex<Option<Engine>> {
    ENGINE.get_or_init(|| Mutex::new(None))
}

/// Run a closure with the engine, mapping any error into a bridge error.
fn with_engine<T>(f: impl FnOnce(&Engine) -> anyhow::Result<T>) -> Result<T, BridgeError> {
    let guard = engine_slot().lock().map_err(|_| err("engine lock poisoned"))?;
    let engine = guard.as_ref().ok_or_else(|| err("engine is not initialized"))?;
    f(engine).map_err(|e| err(format!("{e:#}")))
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeInit<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    context: JObject<'local>,
    output_dir: JString<'local>,
    state_dir: JString<'local>,
) -> jboolean {
    unowned
        .with_env(|env| -> Result<jboolean, BridgeError> {
            let output = PathBuf::from(output_dir.to_string());
            let state = PathBuf::from(state_dir.to_string());
            // Route Rust logs into logcat under tag "HoriCore".
            let _ = android_logger::init_once(
                android_logger::Config::default()
                    .with_max_level(log::LevelFilter::Debug)
                    .with_tag("HoriCore"),
            );
            // librqbit logs through tracing; bridge those events into log.
            let _ = tracing_log::LogTracer::init();
            // reqwest's rustls backend verifies certificates through the
            // Android platform verifier; it must receive the JVM context
            // before any HTTPS request is made.
            rustls_platform_verifier::android::init_with_env(env, context)
                .map_err(|e| err(format!("TLS verifier init failed: {e}")))?;
            let engine = Engine::new(output, Some(state)).map_err(|e| err(format!("{e:#}")))?;
            let mut guard = engine_slot().lock().unwrap();
            *guard = Some(engine);
            Ok(JNI_TRUE)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeAdd<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    source: JString<'local>,
    paused: jboolean,
) -> jlong {
    unowned
        .with_env(|_env| -> Result<jlong, BridgeError> {
            let source = source.to_string();
            with_engine(|engine| Ok(engine.add(&source, paused == JNI_TRUE)? as jlong))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeAddBytes<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    bytes: JByteArray<'local>,
    paused: jboolean,
) -> jlong {
    unowned
        .with_env(|env| -> Result<jlong, BridgeError> {
            let data = env.convert_byte_array(&bytes).map_err(err)?;
            with_engine(|engine| Ok(engine.add_bytes(data, paused == JNI_TRUE)? as jlong))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeList<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<JString, BridgeError> {
            let json = with_engine(|engine| engine.list_json())?;
            env.new_string(json).map_err(err)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativePause<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    id: jlong,
) -> jboolean {
    unowned
        .with_env(|_env| -> Result<jboolean, BridgeError> {
            with_engine(|engine| {
                engine.pause(id as usize)?;
                Ok(JNI_TRUE)
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeResume<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    id: jlong,
) -> jboolean {
    unowned
        .with_env(|_env| -> Result<jboolean, BridgeError> {
            with_engine(|engine| {
                engine.resume(id as usize)?;
                Ok(JNI_TRUE)
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeDelete<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    id: jlong,
    delete_files: jboolean,
) -> jboolean {
    unowned
        .with_env(|_env| -> Result<jboolean, BridgeError> {
            with_engine(|engine| {
                engine.delete(id as usize, delete_files == JNI_TRUE)?;
                Ok(JNI_TRUE)
            })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeTotalSpeed<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<JString, BridgeError> {
            // "down,up" in bytes per second.
            let (down, up) = with_engine(|engine| Ok(engine.total_speed()))?;
            env.new_string(format!("{down},{up}")).map_err(err)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeVersion<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    unowned
        .with_env(|env| -> Result<JString, BridgeError> {
            let v = format!(
                "hori-core {} / librqbit {}",
                env!("CARGO_PKG_VERSION"),
                librqbit::version()
            );
            env.new_string(v).map_err(err)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeShutdown<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
) {
    let _ = unowned.with_env(|_env| -> Result<(), BridgeError> {
        with_engine(|engine| {
            engine.block_on(async {
                // Give in-flight persistence writes a moment to finish.
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            });
            Ok(())
        })?;
        if let Ok(mut guard) = engine_slot().lock() {
            *guard = None;
        }
        Ok(())
    });
}
