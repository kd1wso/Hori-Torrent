//! JNI bridge: exposes the engine to `dev.hori.torrent.NativeBridge` (Kotlin).
//! Every method returns plain data (String JSON / long id / boolean); errors are
//! thrown as `java.lang.RuntimeException` with a readable message.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use jni::objects::{JByteArray, JClass, JObject, JString};
use jni::sys::{jboolean, jlong, JNI_TRUE};
use jni::JNIEnv;

use crate::engine::Engine;

static ENGINE: OnceLock<Mutex<Option<Engine>>> = OnceLock::new();

fn engine_slot() -> &'static Mutex<Option<Engine>> {
    ENGINE.get_or_init(|| Mutex::new(None))
}

/// Run a closure with the engine; on any error throw a RuntimeException and
/// return `default`. Panics are caught too, so a Rust bug never kills the JVM.
fn with_engine<R>(env: &mut JNIEnv, default: R, f: impl FnOnce(&Engine) -> anyhow::Result<R>) -> R {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let guard = engine_slot()
            .lock()
            .map_err(|_| anyhow::anyhow!("engine lock poisoned"))?;
        let engine = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("engine is not initialized"))?;
        f(engine)
    }));
    match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("{e:#}"));
            default
        }
        Err(_) => {
            let _ = env.throw_new("java/lang/RuntimeException", "native panic in hori-core");
            default
        }
    }
}

fn jstring_to_string(env: &mut JNIEnv, s: &JString) -> anyhow::Result<String> {
    Ok(env.get_string(s)?.into())
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeInit(
    mut env: JNIEnv,
    _class: JClass,
    context: JObject,
    output_dir: JString,
    state_dir: JString,
) -> jboolean {
    let setup = (|| -> anyhow::Result<()> {
        let output = PathBuf::from(jstring_to_string(&mut env, &output_dir)?);
        let state = PathBuf::from(jstring_to_string(&mut env, &state_dir)?);
        // Route Rust logs into logcat under tag "HoriCore".
        let _ = android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Debug)
                .with_tag("HoriCore"),
        );
        // Forward tracing events (librqbit uses tracing) into log as well.
        use tracing_subscriber::layer::SubscriberExt;
        let _ = tracing::subscriber::set_global_default(
            tracing_subscriber::registry()
                .with(tracing_log::LogTracer::new())
                .with(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| "info,librqbit=debug,librqbit_dht=info".into()),
                ),
        );
        // reqwest's rustls backend verifies certificates through the Android
        // platform verifier; it must receive the JVM context before any
        // HTTPS request is made.
        rustls_platform_verifier::android::init_with_env(&mut env, context)
            .map_err(|e| anyhow::anyhow!("TLS verifier init failed: {e}"))?;
        let engine = Engine::new(output, Some(state))?;
        let mut guard = engine_slot().lock().unwrap();
        *guard = Some(engine);
        Ok(())
    })();
    match setup {
        Ok(()) => JNI_TRUE,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("{e:#}"));
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeAdd(
    mut env: JNIEnv,
    _class: JClass,
    source: JString,
    paused: jboolean,
) -> jlong {
    let source = match jstring_to_string(&mut env, &source) {
        Ok(s) => s,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", format!("{e:#}"));
            return -1;
        }
    };
    with_engine(&mut env, -1, |engine| {
        Ok(engine.add(&source, paused == JNI_TRUE)? as jlong)
    })
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeAddBytes(
    mut env: JNIEnv,
    _class: JClass,
    bytes: JByteArray,
    paused: jboolean,
) -> jlong {
    let data = match env.convert_byte_array(&bytes) {
        Ok(b) => b,
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", e.to_string());
            return -1;
        }
    };
    with_engine(&mut env, -1, |engine| {
        Ok(engine.add_bytes(data, paused == JNI_TRUE)? as jlong)
    })
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeList<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JObject<'local> {
    let json = with_engine(&mut env, String::from("[]"), |engine| engine.list_json());
    env.new_string(json)
        .map(JObject::from)
        .unwrap_or(JObject::null())
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativePause(
    mut env: JNIEnv,
    _class: JClass,
    id: jlong,
) -> jboolean {
    with_engine(&mut env, 0, |engine| {
        engine.pause(id as usize)?;
        Ok(JNI_TRUE)
    })
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeResume(
    mut env: JNIEnv,
    _class: JClass,
    id: jlong,
) -> jboolean {
    with_engine(&mut env, 0, |engine| {
        engine.resume(id as usize)?;
        Ok(JNI_TRUE)
    })
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeDelete(
    mut env: JNIEnv,
    _class: JClass,
    id: jlong,
    delete_files: jboolean,
) -> jboolean {
    with_engine(&mut env, 0, |engine| {
        engine.delete(id as usize, delete_files == JNI_TRUE)?;
        Ok(JNI_TRUE)
    })
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeTotalSpeed<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JObject<'local> {
    // Returns String "down,up" in bytes per second.
    let (down, up) = with_engine(&mut env, (0u64, 0u64), |engine| Ok(engine.total_speed()));
    env.new_string(format!("{down},{up}"))
        .map(JObject::from)
        .unwrap_or(JObject::null())
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeVersion<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> JObject<'local> {
    let v = format!(
        "hori-core {} / librqbit {}",
        env!("CARGO_PKG_VERSION"),
        librqbit::version()
    );
    env.new_string(v)
        .map(JObject::from)
        .unwrap_or(JObject::null())
}

#[no_mangle]
pub extern "system" fn Java_dev_hori_torrent_NativeBridge_nativeShutdown(
    mut env: JNIEnv,
    _class: JClass,
) {
    let _ = with_engine(&mut env, (), |engine| {
        engine.block_on(async {
            // librqbit persists session state periodically; a short grace
            // period lets in-flight writes finish before the process exits.
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        });
        Ok(())
    });
    if let Ok(mut guard) = engine_slot().lock() {
        *guard = None;
    }
}
