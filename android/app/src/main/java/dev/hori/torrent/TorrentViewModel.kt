package dev.hori.torrent

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

class TorrentViewModel(app: Application) : AndroidViewModel(app) {

    private val _torrents = MutableStateFlow<List<Torrent>>(emptyList())
    val torrents: StateFlow<List<Torrent>> = _torrents

    private val _speed = MutableStateFlow(0L to 0L)
    val speed: StateFlow<Pair<Long, Long>> = _speed

    private val _message = MutableStateFlow<String?>(null)
    val message: StateFlow<String?> = _message

    val outputDir: File = File(app.getExternalFilesDir(null), "downloads").apply { mkdirs() }
    private val stateDir: File = File(app.filesDir, "session").apply { mkdirs() }

    @Volatile var ready = false
        private set

    init {
        viewModelScope.launch {
            try {
                withContext(Dispatchers.IO) {
                    NativeBridge.nativeInit(app.applicationContext, outputDir.absolutePath, stateDir.absolutePath)
                }
                ready = true
                TorrentService.start(app)
            } catch (e: Throwable) {
                _message.value = "Не удалось запустить движок: ${e.message}"
            }
        }
        viewModelScope.launch(Dispatchers.IO) {
            while (isActive) {
                if (ready) {
                    try {
                        _torrents.value = Torrent.parseList(NativeBridge.nativeList())
                        val parts = NativeBridge.nativeTotalSpeed().split(",")
                        _speed.value = parts[0].toLong() to parts[1].toLong()
                    } catch (_: Throwable) {
                    }
                }
                delay(1000)
            }
        }
    }

    fun add(source: String) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val id = NativeBridge.nativeAdd(source.trim(), false)
                _message.value = "Торрент добавлен (id $id)"
            } catch (e: Throwable) {
                _message.value = "Ошибка: ${e.message}"
            }
        }
    }

    fun addBytes(bytes: ByteArray) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val id = NativeBridge.nativeAddBytes(bytes, false)
                _message.value = "Торрент добавлен (id $id)"
            } catch (e: Throwable) {
                _message.value = "Ошибка: ${e.message}"
            }
        }
    }

    fun togglePause(t: Torrent) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                if (t.state == "paused") NativeBridge.nativeResume(t.id)
                else NativeBridge.nativePause(t.id)
            } catch (e: Throwable) {
                _message.value = "Ошибка: ${e.message}"
            }
        }
    }

    fun delete(t: Torrent, withFiles: Boolean) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                NativeBridge.nativeDelete(t.id, withFiles)
                _message.value = "Удалено"
            } catch (e: Throwable) {
                _message.value = "Ошибка: ${e.message}"
            }
        }
    }

    fun consumeMessage() {
        _message.value = null
    }
}
