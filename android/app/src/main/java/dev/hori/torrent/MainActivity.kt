package dev.hori.torrent

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {

    private val viewModel: TorrentViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Ask for notification permission (Android 13+) for the foreground service.
        if (Build.VERSION.SDK_INT >= 33) {
            lifecycleScope.launch {
                val perm = android.Manifest.permission.POST_NOTIFICATIONS
                if (checkSelfPermission(perm) != android.content.pm.PackageManager.PERMISSION_GRANTED) {
                    requestPermissions(arrayOf(perm), 1)
                }
            }
        }

        setContent {
            MaterialTheme {
                Surface(Modifier.fillMaxSize()) {
                    AppScreen(viewModel)
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleIntent(intent)
    }

    override fun onStart() {
        super.onStart()
        handleIntent(intent)
    }

    /** Handle magnet: links and .torrent content URIs the app was opened with. */
    private fun handleIntent(intent: Intent?) {
        if (intent == null) return
        when (intent.action) {
            Intent.ACTION_VIEW -> {
                val data: Uri? = intent.data
                when (data?.scheme) {
                    "magnet" -> viewModel.add(data.toString())
                    "content" -> {
                        val mime = intent.type
                        if (mime == "application/x-bittorrent" ||
                            data.lastPathSegment?.endsWith(".torrent") == true
                        ) {
                            contentResolver.openInputStream(data)?.use { input ->
                                val bytes = input.readBytes()
                                viewModel.addBytes(bytes)
                            }
                        }
                    }
                }
            }
            Intent.ACTION_SEND -> {
                val text = intent.getStringExtra(Intent.EXTRA_TEXT)
                if (text != null && (text.startsWith("magnet:") || text.startsWith("http"))) {
                    viewModel.add(text)
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AppScreen(vm: TorrentViewModel) {
    var showAdd by remember { mutableStateOf(false) }
    var deletedTarget by remember { mutableStateOf<Torrent?>(null) }
    val torrents by vm.torrents.collectAsState()
    val speed by vm.speed.collectAsState()
    val message by vm.message.collectAsState()
    val snackbar = remember { SnackbarHostState() }

    LaunchedEffect(message) {
        message?.let {
            snackbar.showSnackbar(it)
            vm.consumeMessage()
        }
    }

    Scaffold(
        snackbarHost = { SnackbarHost(snackbar) },
        topBar = {
            TopAppBar(
                title = {
                    Column {
                        Text("Hori Torrent", style = MaterialTheme.typography.titleLarge)
                        Text(
                            "↓ ${formatSpeed(speed.first)}   ↑ ${formatSpeed(speed.second)}",
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                },
                actions = {
                    IconButton(onClick = { showAdd = true }) {
                        Icon(Icons.Filled.Add, contentDescription = "Добавить")
                    }
                },
            )
        },
        floatingActionButton = {
            ExtendedFloatingActionButton(
                onClick = { showAdd = true },
                icon = { Icon(Icons.Filled.Add, contentDescription = null) },
                text = { Text("Добавить") },
            )
        },
    ) { padding ->
        if (torrents.isEmpty()) {
            Box(
                Modifier
                    .fillMaxSize()
                    .padding(padding),
                contentAlignment = androidx.compose.ui.Alignment.Center,
            ) {
                Column(horizontalAlignment = androidx.compose.ui.Alignment.CenterHorizontally) {
                    Text("Нет торрентов", style = MaterialTheme.typography.titleMedium)
                    Text(
                        "Добавьте magnet-ссылку или .torrent файл",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        } else {
            LazyColumn(
                Modifier
                    .fillMaxSize()
                    .padding(padding),
                verticalArrangement = Arrangement.spacedBy(8.dp),
                contentPadding = PaddingValues(16.dp),
            ) {
                items(torrents, key = { it.id }) { t ->
                    TorrentCard(
                        torrent = t,
                        onToggle = { vm.togglePause(t) },
                        onDelete = { deletedTarget = t },
                    )
                }
            }
        }
    }

    if (showAdd) {
        AddTorrentDialog(onDismiss = { showAdd = false }) { source ->
            vm.add(source)
            showAdd = false
        }
    }

    deletedTarget?.let { t ->
        AlertDialog(
            onDismissRequest = { deletedTarget = null },
            title = { Text("Удалить «${t.name ?: "торрент"}»?") },
            text = { Text("Загруженные файлы останутся на диске.") },
            confirmButton = {
                TextButton(onClick = {
                    vm.delete(t, withFiles = false)
                    deletedTarget = null
                }) { Text("Удалить") }
            },
            dismissButton = {
                TextButton(onClick = { deletedTarget = null }) { Text("Отмена") }
            },
            icon = { Icon(Icons.Filled.Delete, contentDescription = null) },
        )
    }
}

@Composable
fun TorrentCard(torrent: Torrent, onToggle: () -> Unit, onDelete: () -> Unit) {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    torrent.name ?: "(без имени)",
                    style = MaterialTheme.typography.titleMedium,
                    modifier = Modifier.weight(1f, fill = false),
                    maxLines = 2,
                )
                Text(
                    torrent.state,
                    color = when (torrent.state) {
                        "downloading" -> MaterialTheme.colorScheme.primary
                        "seeding" -> MaterialTheme.colorScheme.tertiary
                        "paused" -> MaterialTheme.colorScheme.onSurfaceVariant
                        "error" -> MaterialTheme.colorScheme.error
                        else -> MaterialTheme.colorScheme.onSurfaceVariant
                    },
                    style = MaterialTheme.typography.labelMedium,
                )
            }

            if (torrent.error != null) {
                Text(
                    torrent.error,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall,
                    maxLines = 2,
                )
            }

            LinearProgressIndicator(
                progress = { torrent.progressFraction },
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 6.dp),
            )

            Text(
                "${formatBytes(torrent.progressBytes)} / ${formatBytes(torrent.totalBytes)}" +
                    "   ↓ ${formatSpeed(torrent.downloadBps)}" +
                    (if (torrent.uploadBps > 0) "   ↑ ${formatSpeed(torrent.uploadBps)}" else "") +
                    "   пиры: ${torrent.peers}",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
            ) {
                IconButton(onClick = onToggle) {
                    Icon(
                        if (torrent.state == "paused") Icons.Filled.PlayArrow else Icons.Filled.Close,
                        contentDescription = if (torrent.state == "paused") "Продолжить" else "Пауза",
                    )
                }
                IconButton(onClick = onDelete) {
                    Icon(Icons.Filled.Delete, contentDescription = "Удалить")
                }
            }
        }
    }
}

@Composable
fun AddTorrentDialog(onDismiss: () -> Unit, onAdd: (String) -> Unit) {
    var text by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Добавить торрент") },
        text = {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                label = { Text("magnet-ссылка или URL .torrent") },
                singleLine = false,
                modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = {
            TextButton(
                onClick = { if (text.isNotBlank()) onAdd(text) },
                enabled = text.isNotBlank(),
            ) { Text("Добавить") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Отмена") }
        },
    )
}
