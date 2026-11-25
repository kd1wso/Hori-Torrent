package dev.hori.torrent

import org.json.JSONArray

data class TorrentFile(
    val name: String,
    val length: Long,
    val progress: Long,
    val included: Boolean,
)

data class Torrent(
    val id: Long,
    val name: String?,
    val state: String,
    val error: String?,
    val totalBytes: Long,
    val progressBytes: Long,
    val uploadedBytes: Long,
    val downloadBps: Long,
    val uploadBps: Long,
    val peers: Int,
    val finished: Boolean,
    val outputFolder: String,
    val files: List<TorrentFile>,
) {
    val progressFraction: Float
        get() = if (totalBytes > 0) progressBytes.toFloat() / totalBytes else 0f

    companion object {
        fun parseList(json: String): List<Torrent> {
            val arr = JSONArray(json)
            return (0 until arr.length()).map { i ->
                val o = arr.getJSONObject(i)
                val filesArr = o.optJSONArray("files") ?: JSONArray()
                Torrent(
                    id = o.getLong("id"),
                    name = o.optString("name").takeIf { it.isNotEmpty() && it != "null" },
                    state = o.getString("state"),
                    error = o.optString("error").takeIf { it.isNotEmpty() && it != "null" },
                    totalBytes = o.getLong("total_bytes"),
                    progressBytes = o.getLong("progress_bytes"),
                    uploadedBytes = o.getLong("uploaded_bytes"),
                    downloadBps = o.getLong("download_bps"),
                    uploadBps = o.getLong("upload_bps"),
                    peers = o.getInt("peers"),
                    finished = o.getBoolean("finished"),
                    outputFolder = o.getString("output_folder"),
                    files = (0 until filesArr.length()).map { j ->
                        val f = filesArr.getJSONObject(j)
                        TorrentFile(
                            name = f.getString("name"),
                            length = f.getLong("length"),
                            progress = f.optLong("progress", 0),
                            included = f.optBoolean("included", true),
                        )
                    },
                )
            }
        }
    }
}

fun formatBytes(b: Long): String {
    if (b < 1024) return "$b Б"
    val units = listOf("КиБ", "МиБ", "ГиБ", "ТиБ")
    var v = b.toDouble()
    var i = -1
    while (v >= 1024 && i < units.size - 1) {
        v /= 1024
        i++
    }
    return String.format("%.1f %s", v, units[i])
}

fun formatSpeed(bps: Long): String = "${formatBytes(bps)}/с"
