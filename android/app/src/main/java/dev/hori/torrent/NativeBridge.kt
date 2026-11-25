package dev.hori.torrent

/**
 * Thin Kotlin mirror of `src/jni_bridge.rs`. All methods are static (Rust side
 * takes JClass). The native library is `libhori_core.so`, built by cargo-ndk.
 */
object NativeBridge {
    init {
        System.loadLibrary("hori_core")
    }

    @JvmStatic external fun nativeInit(context: android.content.Context, outputDir: String, stateDir: String): Boolean
    @JvmStatic external fun nativeAdd(source: String, paused: Boolean): Long
    @JvmStatic external fun nativeAddBytes(bytes: ByteArray, paused: Boolean): Long
    @JvmStatic external fun nativeList(): String
    @JvmStatic external fun nativePause(id: Long): Boolean
    @JvmStatic external fun nativeResume(id: Long): Boolean
    @JvmStatic external fun nativeDelete(id: Long, deleteFiles: Boolean): Boolean
    @JvmStatic external fun nativeTotalSpeed(): String
    @JvmStatic external fun nativeVersion(): String
    @JvmStatic external fun nativeShutdown()
}
