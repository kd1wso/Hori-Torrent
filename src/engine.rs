use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use librqbit::api::TorrentIdOrHash;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, Session, SessionOptions,
    SessionPersistenceConfig, TorrentStatsState,
};
use serde::Serialize;
use tokio::runtime::Runtime;

/// A JSON-friendly snapshot of one torrent, consumed by the Android UI.
#[derive(Serialize, Clone, Debug)]
pub struct TorrentInfo {
    pub id: usize,
    pub name: Option<String>,
    pub state: String,
    pub error: Option<String>,
    pub total_bytes: u64,
    pub progress_bytes: u64,
    pub uploaded_bytes: u64,
    /// Bytes per second.
    pub download_bps: u64,
    pub upload_bps: u64,
    pub peers: u32,
    pub finished: bool,
    pub output_folder: String,
    pub files: Vec<FileInfo>,
}

#[derive(Serialize, Clone, Debug)]
pub struct FileInfo {
    pub name: String,
    pub length: u64,
    pub progress: u64,
    pub included: bool,
}

/// Owns a multi-threaded tokio runtime and a librqbit session.
/// All public methods are synchronous so they can be called from JNI threads.
pub struct Engine {
    rt: Runtime,
    session: Arc<Session>,
    output_folder: PathBuf,
}

impl Engine {
    /// Create an engine storing downloads in `output_folder`.
    /// `session_folder` (optional) keeps resume state between app restarts.
    pub fn new(output_folder: PathBuf, session_folder: Option<PathBuf>) -> Result<Self> {
        std::fs::create_dir_all(&output_folder)
            .with_context(|| format!("cannot create output folder {}", output_folder.display()))?;
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .context("cannot start tokio runtime")?;
        let session = rt.block_on(async {
            let mut opts = SessionOptions::default();
            opts.fastresume = true;
            if let Some(folder) = session_folder {
                std::fs::create_dir_all(&folder)?;
                opts.persistence = Some(SessionPersistenceConfig::Json {
                    folder: Some(folder),
                });
            }
            Session::new_with_opts(output_folder.clone(), opts).await
        })?;
        Ok(Self {
            rt,
            session,
            output_folder,
        })
    }

    pub fn output_folder(&self) -> &Path {
        &self.output_folder
    }

    /// Add a torrent from a magnet link, an http(s) URL to a .torrent file,
    /// or a local .torrent file path. Returns the assigned id.
    pub fn add(&self, source: &str, paused: bool) -> Result<usize> {
        let add = if source.starts_with("magnet:")
            || source.starts_with("http://")
            || source.starts_with("https://")
        {
            AddTorrent::from_url(source)
        } else {
            let bytes = std::fs::read(source)
                .with_context(|| format!("cannot read torrent file {source}"))?;
            AddTorrent::from_bytes(bytes)
        };
        self.add_inner(add, paused)
    }

    /// Add a torrent from raw .torrent file bytes (e.g. picked via SAF on Android).
    pub fn add_bytes(&self, bytes: Vec<u8>, paused: bool) -> Result<usize> {
        self.add_inner(AddTorrent::from_bytes(bytes), paused)
    }

    fn add_inner(&self, add: AddTorrent<'_>, paused: bool) -> Result<usize> {
        let opts = AddTorrentOptions {
            paused,
            overwrite: true,
            output_folder: Some(self.output_folder.to_string_lossy().into_owned()),
            ..Default::default()
        };
        self.rt.block_on(async {
            let resp = self
                .session
                .add_torrent(add, Some(opts))
                .await
                .context("add_torrent failed")?;
            match resp {
                AddTorrentResponse::Added(id, _) => Ok(id),
                AddTorrentResponse::AlreadyManaged(id, _) => Ok(id),
                AddTorrentResponse::ListOnly(_) => {
                    Err(anyhow!("torrent was not added (list-only)"))
                }
            }
        })
    }

    pub fn pause(&self, id: usize) -> Result<()> {
        let handle = self
            .session
            .get(TorrentIdOrHash::Id(id))
            .ok_or_else(|| anyhow!("no torrent with id {id}"))?;
        self.rt
            .block_on(self.session.pause(&handle))
            .context("pause failed")
    }

    pub fn resume(&self, id: usize) -> Result<()> {
        let handle = self
            .session
            .get(TorrentIdOrHash::Id(id))
            .ok_or_else(|| anyhow!("no torrent with id {id}"))?;
        self.rt
            .block_on(self.session.unpause(&handle))
            .context("resume failed")
    }

    /// Remove a torrent from the session, optionally deleting downloaded files.
    pub fn delete(&self, id: usize, delete_files: bool) -> Result<()> {
        self.rt
            .block_on(self.session.delete(TorrentIdOrHash::Id(id), delete_files))
            .context("delete failed")
    }

    /// Snapshot of all torrents as a JSON array string.
    pub fn list_json(&self) -> Result<String> {
        // `with_torrents` takes a plain `Fn`, so interior mutability is needed
        // to accumulate rows inside the callback.
        let out = std::cell::RefCell::new(Vec::<TorrentInfo>::new());
        self.session.with_torrents(|iter| {
            let mut out = out.borrow_mut();
            for (id, handle) in iter {
                let stats = handle.stats();
                let (download_bps, upload_bps, peers) = match &stats.live {
                    Some(live) => (
                        live.download_speed.as_bytes(),
                        live.upload_speed.as_bytes(),
                        live.snapshot.peer_stats.live,
                    ),
                    None => (0, 0, 0),
                };
                let state = match &stats.state {
                    TorrentStatsState::Paused => "paused",
                    TorrentStatsState::Error => "error",
                    TorrentStatsState::Initializing { .. } => "initializing",
                    TorrentStatsState::Live => {
                        if stats.finished {
                            "seeding"
                        } else if peers > 0 {
                            "downloading"
                        } else {
                            "waiting"
                        }
                    }
                };
                let only = handle.only_files();
                let files = handle
                    .with_metadata(|m| {
                        m.file_infos
                            .iter()
                            .enumerate()
                            .map(|(idx, f)| FileInfo {
                                name: f
                                    .relative_filename
                                    .components()
                                    .map(|c| c.as_os_str().to_string_lossy())
                                    .collect::<Vec<_>>()
                                    .join("/"),
                                length: f.len,
                                progress: stats.file_progress.get(idx).copied().unwrap_or(0),
                                included: only
                                    .as_ref()
                                    .map(|set| set.contains(&idx))
                                    .unwrap_or(true),
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                out.push(TorrentInfo {
                    id,
                    name: handle.name(),
                    state: state.to_string(),
                    error: stats.error.clone(),
                    total_bytes: stats.total_bytes,
                    progress_bytes: stats.progress_bytes,
                    uploaded_bytes: stats.uploaded_bytes,
                    download_bps,
                    upload_bps,
                    peers,
                    finished: stats.finished,
                    output_folder: handle.output_folder().to_string_lossy().into_owned(),
                    files,
                });
            }
        });
        let mut out = out.into_inner();
        out.sort_by_key(|t| t.id);
        Ok(serde_json::to_string(&out)?)
    }

    /// Total download/upload speed across the session, bytes per second.
    pub fn total_speed(&self) -> (u64, u64) {
        let snap = self.session.stats_snapshot();
        (
            snap.download_speed.as_bytes(),
            snap.upload_speed.as_bytes(),
        )
    }

    pub fn block_on<F: std::future::Future>(&self, f: F) -> F::Output {
        self.rt.block_on(f)
    }

    /// Sleep on the engine runtime (used by the CLI loop).
    pub fn sleep(&self, d: Duration) {
        // The future must be constructed inside the runtime context.
        self.rt.block_on(async move { tokio::time::sleep(d).await });
    }
}
