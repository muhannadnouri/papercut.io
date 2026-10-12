//! Explicit, pinned model-cache downloads. No inference, indexing or implicit network work.
use super::{ModelSelection, ProviderStatus, SearchEmbeddingState};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{BufReader, Read, Write},
    panic::{catch_unwind, AssertUnwindSafe},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::Emitter;

const PROGRESS_EVENT: &str = "search-embedding-download-progress";
use super::models::Asset;

fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("Embedding download cancelled".into())
    } else {
        Ok(())
    }
}

/// Shared integrity boundary for downloaded streams, cached files and loader bytes.
pub(super) fn copy_verified(
    reader: &mut impl Read,
    writer: &mut impl Write,
    bytes: u64,
    sha256: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64),
) -> Result<(), String> {
    let mut hash = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        check_cancel(cancel)?;
        let read = reader.read(&mut buffer).map_err(|err| err.to_string())?;
        if read == 0 {
            break;
        }
        count += read as u64;
        if count > bytes {
            return Err("Incorrect embedding asset size".into());
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|err| err.to_string())?;
        hash.update(&buffer[..read]);
        progress(count);
    }
    check_cancel(cancel)?;
    if count != bytes {
        return Err("Incorrect embedding asset size".into());
    }
    if format!("{:x}", hash.finalize()) != sha256 {
        return Err("Incorrect embedding asset checksum".into());
    }
    Ok(())
}

pub(super) fn files_present(dir: &Path, assets: &[Asset]) -> bool {
    fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir())
        && assets.iter().all(|asset| {
            fs::symlink_metadata(dir.join(asset.path))
                .is_ok_and(|meta| meta.is_file() && meta.len() == asset.bytes)
        })
}

fn verify_cache(dir: &Path, assets: &[Asset], cancel: &AtomicBool) -> Result<(), String> {
    if !files_present(dir, assets) {
        return Err("Embedding cache is incomplete; remove it before downloading again".into());
    }
    for asset in assets {
        let mut file =
            BufReader::new(File::open(dir.join(asset.path)).map_err(|err| err.to_string())?);
        copy_verified(
            &mut file,
            &mut std::io::sink(),
            asset.bytes,
            asset.sha256,
            cancel,
            |_| {},
        )?;
    }
    Ok(())
}

fn staging_dir(dir: &Path) -> PathBuf {
    dir.with_extension("installing")
}

/// Incomplete final/staging caches still need cleanup; completeness is not storage presence.
pub(super) fn has_stored_files(dir: &Path) -> Result<bool, String> {
    for path in [dir.to_path_buf(), staging_dir(dir)] {
        match fs::symlink_metadata(path) {
            Ok(_) => return Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.to_string()),
        }
    }
    Ok(false)
}

struct StagingDir(PathBuf);
impl Drop for StagingDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn remove_dir(dir: &Path) -> Result<(), String> {
    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

/// Keep the final cache absent until every streamed asset passes verification.
fn install(
    dir: &Path,
    assets: &[Asset],
    cancel: &AtomicBool,
    mut fetch: impl FnMut(&Asset, &mut File, &AtomicBool) -> Result<(), String>,
    publish: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    check_cancel(cancel)?;
    if dir.try_exists().map_err(|err| err.to_string())? {
        return verify_cache(dir, assets, cancel);
    }
    let parent = dir.parent().ok_or("Invalid embedding cache path")?;
    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    let stage = staging_dir(dir);
    // ponytail: one app-owned cache mutation at a time; restart interrupted downloads,
    // add resumable transfers only if retry cost warrants maintaining that protocol.
    remove_dir(&stage)?;
    crate::library_transfer::ensure_available_space(
        parent,
        assets.iter().map(|asset| asset.bytes).sum(),
    )
    .map_err(|err| err.to_string())?;
    fs::create_dir(&stage).map_err(|err| err.to_string())?;
    let stage = StagingDir(stage);
    for asset in assets {
        check_cancel(cancel)?;
        let path = stage.0.join(asset.path);
        fs::create_dir_all(path.parent().ok_or("Invalid embedding asset path")?)
            .map_err(|err| err.to_string())?;
        let mut file = File::create(&path).map_err(|err| err.to_string())?;
        fetch(asset, &mut file, cancel)?;
        file.sync_all().map_err(|err| err.to_string())?;
    }
    // Re-read staging as well: the injection boundary must never publish unchecked bytes.
    verify_cache(&stage.0, assets, cancel)?;
    publish(&stage.0)
}

fn downloader() -> Result<reqwest::blocking::Client, String> {
    // Use the existing blocking installer pattern with an idle-body timeout as well.
    reqwest::blocking::ClientBuilder::from(
        reqwest::Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(10)),
    )
    .timeout(Duration::from_secs(30 * 60))
    .user_agent("Papercut experimental search model installer")
    .build()
    .map_err(|err| err.to_string())
}

fn fetch_asset(
    client: &reqwest::blocking::Client,
    url: &str,
    asset: &Asset,
    file: &mut File,
    cancel: &AtomicBool,
    progress: impl FnMut(u64),
) -> Result<(), String> {
    check_cancel(cancel)?;
    // reqwest's idle-body timeout polls a Tokio timer even through blocking Read.
    let runtime = tauri::async_runtime::handle();
    let _context = runtime.inner().enter();
    let mut response = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|err| format!("Embedding download unavailable: {err}"))?;
    if response
        .content_length()
        .is_some_and(|size| size != asset.bytes)
    {
        return Err("Incorrect embedding download size".into());
    }
    copy_verified(
        &mut response,
        file,
        asset.bytes,
        asset.sha256,
        cancel,
        progress,
    )
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    format: ModelSelection,
    phase: &'static str,
    downloaded_bytes: u64,
    total_bytes: u64,
}

impl SearchEmbeddingState {
    fn download_with(
        &self,
        format: ModelSelection,
        task: impl FnOnce(&AtomicBool) -> Result<(), String>,
    ) -> Result<ProviderStatus, String> {
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut inner = self.0.lock().map_err(|_| "Embedding state lock failed")?;
            if inner.downloading.is_some() || inner.loading.is_some() {
                return Err("An embedding activation or download is already running".into());
            }
            inner.downloading = Some((format, cancel.clone()));
        }
        let result = catch_unwind(AssertUnwindSafe(|| task(&cancel)))
            .unwrap_or_else(|_| Err("Embedding download task panicked".into()));
        let mut inner = self.0.lock().map_err(|_| "Embedding state lock failed")?;
        inner.downloading = None;
        let result = if result.is_err() && cancel.load(Ordering::Relaxed) {
            check_cancel(&cancel)
        } else {
            result
        };
        drop(inner);
        result?;
        self.status()
    }

    pub(super) fn download<R: tauri::Runtime>(
        &self,
        app: &tauri::AppHandle<R>,
        dir: &Path,
        format: ModelSelection,
    ) -> Result<ProviderStatus, String> {
        self.download_with(format, |cancel| {
            let total = format.installed_bytes();
            let emit = |phase, downloaded| {
                let progress = DownloadProgress {
                    format,
                    phase,
                    downloaded_bytes: downloaded,
                    total_bytes: total,
                };
                let _ = app.emit(PROGRESS_EVENT, progress);
            };
            emit("starting", 0);
            let mut downloaded = 0;
            let mut last_percent = 0;
            let result = (|| {
                let client = downloader()?;
                install(
                    dir,
                    &format.assets(),
                    cancel,
                    |asset, file, cancel| {
                        let url = format.asset_url(asset);
                        let offset = downloaded;
                        fetch_asset(&client, &url, asset, file, cancel, |count| {
                            let bytes = offset + count;
                            downloaded = bytes;
                            let percent = bytes * 100 / total;
                            if percent >= last_percent + 2 {
                                last_percent = percent;
                                emit("downloading", bytes);
                            }
                        })?;
                        Ok(())
                    },
                    |stage| self.publish(stage, dir, cancel),
                )
            })();
            let phase = if result.is_ok() {
                "installed"
            } else if cancel.load(Ordering::Relaxed) {
                "cancelled"
            } else {
                "failed"
            };
            emit(phase, if result.is_ok() { total } else { downloaded });
            result
        })
    }

    fn publish(&self, stage: &Path, dir: &Path, cancel: &AtomicBool) -> Result<(), String> {
        let _inner = self.0.lock().map_err(|_| "Embedding state lock failed")?;
        check_cancel(cancel)?;
        fs::rename(stage, dir).map_err(|err| err.to_string())
    }

    pub(super) fn remove(
        &self,
        dir: &Path,
        format: ModelSelection,
    ) -> Result<ProviderStatus, String> {
        let inner = self.0.lock().map_err(|_| "Embedding state lock failed")?;
        if inner.loading.is_some() || inner.downloading.is_some() {
            return Err(
                "Wait for the embedding activation or download to finish before removal".into(),
            );
        }
        if inner
            .active
            .as_ref()
            .is_some_and(|loaded| loaded.format == format)
        {
            return Err("Disable embeddings before removing the active model".into());
        }
        remove_dir(dir)?;
        remove_dir(&staging_dir(dir))?;
        drop(inner);
        self.status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, Cursor};

    const SMALL: Asset = Asset {
        path: "onnx/model.onnx",
        upstream_path: "onnx/model.onnx",
        bytes: 3,
        sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    };

    #[test]
    fn stored_files_include_partial_caches_and_staging_until_removed() {
        let root = work("storage-presence");
        let dir = root.0.join("cache");
        assert!(!has_stored_files(&dir).unwrap());
        fs::create_dir_all(staging_dir(&dir)).unwrap();
        fs::write(staging_dir(&dir).join("partial"), b"partial").unwrap();
        assert!(has_stored_files(&dir).unwrap());
        assert!(!files_present(&dir, &[SMALL]));
        remove_dir(&staging_dir(&dir)).unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("incomplete"), b"partial").unwrap();
        assert!(has_stored_files(&dir).unwrap());
        assert!(!files_present(&dir, &[SMALL]));
        remove_dir(&dir).unwrap();
        assert!(!has_stored_files(&dir).unwrap());
    }

    fn work(name: &str) -> StagingDir {
        let path = std::env::temp_dir().join(format!(
            "papercut-embedding-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        StagingDir(path)
    }

    #[test]
    fn pinned_cache_checks_integrity_cleanup_reuse_cancellation_and_removal() {
        assert_eq!(ModelSelection::Float32.installed_bytes(), 487_352_505);
        assert_eq!(ModelSelection::DynamicInt8.installed_bytes(), 135_430_819);
        let root = work("lifecycle");
        let dir = root.0.join("float32");
        let assets = [
            Asset {
                path: "config.json",
                ..SMALL
            },
            SMALL,
        ];
        let cancel = AtomicBool::new(false);
        let state = SearchEmbeddingState::default();
        assert!(copy_verified(
            &mut Cursor::new(b"abc"),
            &mut Vec::new(),
            SMALL.bytes,
            SMALL.sha256,
            &cancel,
            |_| cancel.store(true, Ordering::Relaxed),
        )
        .unwrap_err()
        .contains("cancelled"));
        cancel.store(false, Ordering::Relaxed);
        assert!(install(
            &dir,
            &assets,
            &cancel,
            |_, file, _| { file.write_all(b"abd").map_err(|err| err.to_string()) },
            |_| panic!("unchecked staging must not publish")
        )
        .is_err());
        assert!(!dir.exists());
        assert!(!staging_dir(&dir).exists());
        for bad in [b"ab".as_slice(), b"abcd", b"abd"] {
            let result = install(
                &dir,
                &assets,
                &cancel,
                |asset, file, cancel| {
                    copy_verified(
                        &mut Cursor::new(bad),
                        file,
                        asset.bytes,
                        asset.sha256,
                        cancel,
                        |_| {},
                    )
                },
                |stage| state.publish(stage, &dir, &cancel),
            );
            assert!(result.is_err());
            assert!(!dir.exists());
            assert!(!staging_dir(&dir).exists());
        }
        let fetch = |asset: &Asset, file: &mut File, cancel: &AtomicBool| {
            copy_verified(
                &mut Cursor::new(b"abc"),
                file,
                asset.bytes,
                asset.sha256,
                cancel,
                |_| {},
            )
        };
        let result = install(&dir, &assets, &cancel, fetch, |stage| {
            cancel.store(true, Ordering::Relaxed);
            state.publish(stage, &dir, &cancel)
        });
        assert!(result.unwrap_err().contains("cancelled"));
        assert!(!dir.exists());
        assert!(!staging_dir(&dir).exists());
        cancel.store(false, Ordering::Relaxed);
        fs::create_dir(staging_dir(&dir)).unwrap();
        fs::write(staging_dir(&dir).join("interrupted"), b"partial").unwrap();
        install(&dir, &assets, &cancel, fetch, |stage| {
            state.publish(stage, &dir, &cancel)
        })
        .unwrap();
        assert!(files_present(&dir, &assets));
        assert!(!staging_dir(&dir).exists());
        install(
            &dir,
            &assets,
            &cancel,
            |_, _, _| panic!("verified cache must not fetch"),
            |_| panic!("verified cache must not publish"),
        )
        .unwrap();
        fs::write(dir.join("config.json"), b"abd").unwrap();
        assert!(files_present(&dir, &assets)); // Presence is explicitly not integrity.
        assert!(install(
            &dir,
            &assets,
            &cancel,
            |_, _, _| panic!("invalid cache needs explicit removal"),
            |_| panic!()
        )
        .is_err());
        assert_eq!(fs::read(dir.join("config.json")).unwrap(), b"abd");
        state.remove(&dir, ModelSelection::Float32).unwrap();
        state.remove(&dir, ModelSelection::Float32).unwrap();
        let oversized = [Asset {
            bytes: u64::MAX,
            ..SMALL
        }];
        assert!(install(
            &dir,
            &oversized,
            &cancel,
            |_, _, _| panic!("storage preflight must precede fetching"),
            |_| panic!()
        )
        .is_err());
        assert!(!dir.exists());
        assert!(!staging_dir(&dir).exists());
        assert!(state.status().unwrap().active_format.is_none());
    }

    #[test]
    fn pending_download_blocks_activation_and_removal_until_cancelled_cleanup_finishes() {
        let root = work("busy");
        let dir = root.0.join("float32");
        let state = SearchEmbeddingState::default();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let worker_state = state.clone();
            let worker_dir = dir.clone();
            let worker = scope.spawn(move || {
                let state = worker_state;
                let dir = worker_dir;
                state.download_with(ModelSelection::Float32, |cancel| {
                    install(
                        &dir,
                        &[SMALL],
                        cancel,
                        |_, file, cancel| {
                            file.write_all(b"a").unwrap();
                            ready_tx.send(()).unwrap();
                            resume_rx.recv().unwrap();
                            check_cancel(cancel)
                        },
                        |stage| state.publish(stage, &dir, cancel),
                    )
                })
            });
            ready_rx.recv().unwrap();
            assert_eq!(
                state.status().unwrap().downloading_format,
                Some(ModelSelection::Float32)
            );
            assert!(state
                .download_with(ModelSelection::DynamicInt8, |_| panic!(
                    "duplicate download"
                ))
                .is_err());
            assert!(state
                .activate(Path::new("missing"), ModelSelection::Float32)
                .unwrap_err()
                .contains("already running"));
            assert!(state.remove(&dir, ModelSelection::Float32).is_err());
            state.disable().unwrap();
            assert!(state.remove(&dir, ModelSelection::Float32).is_err());
            resume_tx.send(()).unwrap();
            assert!(worker.join().unwrap().unwrap_err().contains("cancelled"));
        });
        assert!(state.status().unwrap().downloading_format.is_none());
        assert!(!dir.exists());
        assert!(!staging_dir(&dir).exists());
        assert!(state
            .download_with(ModelSelection::Float32, |_| {
                install(
                    &dir,
                    &[SMALL],
                    &AtomicBool::new(false),
                    |_, _, _| panic!("injected failure"),
                    |_| panic!(),
                )
            })
            .is_err());
        assert!(state.status().unwrap().downloading_format.is_none());
        assert!(!staging_dir(&dir).exists());
        assert!(state
            .download_with(ModelSelection::Float32, |_| Err("offline".into()))
            .is_err());
        assert!(state.status().unwrap().downloading_format.is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn process_interruption_recovers_staging_and_reuses_published_cache() {
        use std::{os::unix::process::ExitStatusExt, process::Command, time::Instant};

        let assets = [
            Asset {
                path: "config.json",
                ..SMALL
            },
            SMALL,
        ];
        let state = SearchEmbeddingState::default();
        if let Some(root) = std::env::var_os("PAPERCUT_EMBEDDING_INTERRUPTION_ROOT") {
            let root = fs::canonicalize(root).unwrap();
            assert!(root.starts_with(fs::canonicalize(std::env::temp_dir()).unwrap()));
            assert!(root
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("papercut-embedding-interruption-"));
            let phase = std::env::var("PAPERCUT_EMBEDDING_INTERRUPTION_PHASE").unwrap();
            assert!(matches!(
                phase.as_str(),
                "partial" | "verified" | "published" | "recover"
            ));
            let dir = root.join("float32");
            let reuse = dir.exists();
            let mut fetched = 0;
            let pause = || {
                let ready = root.join("ready");
                fs::write(ready.with_extension("tmp"), phase.as_bytes()).unwrap();
                fs::rename(ready.with_extension("tmp"), ready).unwrap();
                loop {
                    std::thread::park();
                }
            };
            let status = state
                .download_with(ModelSelection::Float32, |cancel| {
                    install(
                        &dir,
                        &assets,
                        cancel,
                        |asset, file, _| {
                            fetched += 1;
                            let partial = phase == "partial" && asset.path == SMALL.path;
                            file.write_all(if partial { b"a" } else { b"abc" }).unwrap();
                            file.sync_all().unwrap();
                            if partial {
                                pause();
                            }
                            Ok(())
                        },
                        |stage| {
                            if phase == "verified" {
                                pause();
                            }
                            state.publish(stage, &dir, cancel)?;
                            if phase == "published" {
                                pause();
                            }
                            Ok(())
                        },
                    )
                })
                .unwrap();
            assert_eq!(phase, "recover");
            assert_eq!(fetched, if reuse { 0 } else { assets.len() });
            assert!(status.active_format.is_none() && status.downloading_format.is_none());
            verify_cache(&dir, &assets, &AtomicBool::new(false)).unwrap();
            assert!(!staging_dir(&dir).exists());
            return;
        }

        // Kill only this test's child. Always reap it, including assertion failures.
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let root = work("interruption");
        let dir = root.0.join("float32");
        let spawn = |phase: &str| {
            let log = File::create(root.0.join(format!("{phase}.log"))).unwrap();
            Child(Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "search_embeddings::cache::tests::process_interruption_recovers_staging_and_reuses_published_cache", "--nocapture"])
                .env("PAPERCUT_EMBEDDING_INTERRUPTION_ROOT", &root.0)
                .env("PAPERCUT_EMBEDDING_INTERRUPTION_PHASE", phase)
                .stdout(log.try_clone().unwrap()).stderr(log).spawn().unwrap())
        };
        for phase in ["partial", "verified", "published"] {
            let mut child = spawn(phase);
            let started = Instant::now();
            while !root.0.join("ready").exists() {
                assert!(
                    child.0.try_wait().unwrap().is_none(),
                    "{}",
                    fs::read_to_string(root.0.join(format!("{phase}.log"))).unwrap()
                );
                assert!(started.elapsed() < Duration::from_secs(10));
                std::thread::sleep(Duration::from_millis(10));
            }
            assert_eq!(fs::read(root.0.join("ready")).unwrap(), phase.as_bytes());
            child.0.kill().unwrap();
            assert_eq!(child.0.wait().unwrap().signal(), Some(9));
            assert_eq!(dir.exists(), phase == "published");
            if phase == "partial" {
                assert_eq!(fs::read(staging_dir(&dir).join(SMALL.path)).unwrap(), b"a");
            } else {
                let cache = if dir.exists() {
                    dir.clone()
                } else {
                    staging_dir(&dir)
                };
                verify_cache(&cache, &assets, &AtomicBool::new(false)).unwrap();
            }
            fs::remove_file(root.0.join("ready")).unwrap();
            let mut restarted = spawn("recover");
            let started = Instant::now();
            let status = loop {
                if let Some(status) = restarted.0.try_wait().unwrap() {
                    break status;
                }
                assert!(started.elapsed() < Duration::from_secs(10));
                std::thread::sleep(Duration::from_millis(10));
            };
            assert!(
                status.success(),
                "{}",
                fs::read_to_string(root.0.join("recover.log")).unwrap()
            );
            verify_cache(&dir, &assets, &AtomicBool::new(false)).unwrap();
            assert!(!staging_dir(&dir).exists());
            eprintln!("SIGKILL at {phase}: fresh-process recovery passed");
            state.remove(&dir, ModelSelection::Float32).unwrap();
        }
    }

    #[test]
    fn http_status_length_checksum_truncation_and_idle_timeout_are_checked() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            for reply in [
                "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc",
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nab",
                "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabd",
                "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nab",
                "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\na",
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                assert!(BufReader::new(&mut stream)
                    .lines()
                    .take(100)
                    .any(|line| line.unwrap().is_empty()));
                stream.write_all(reply.as_bytes()).unwrap();
                if reply.ends_with("\r\n\r\na") {
                    release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
                }
            }
        });
        let client = reqwest::blocking::ClientBuilder::from(
            reqwest::Client::builder()
                .no_proxy()
                .read_timeout(Duration::from_millis(100)),
        )
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
        let root = work("http");
        let cancel = AtomicBool::new(false);
        for index in 0..6 {
            let mut file = File::create(root.0.join("asset")).unwrap();
            let started = std::time::Instant::now();
            let result = fetch_asset(&client, &url, &SMALL, &mut file, &cancel, |_| {});
            if index == 5 {
                release_tx.send(()).unwrap();
                assert!(
                    started.elapsed() < Duration::from_secs(1),
                    "idle read should time out before the two-second total limit"
                );
            }
            assert_eq!(result.is_ok(), index == 0, "response {index}: {result:?}");
        }
        server.join().unwrap();
        assert!(
            downloader().unwrap().get(&url).send().is_err(),
            "production downloader is HTTPS-only"
        );
        let mut file = File::create(root.0.join("asset")).unwrap();
        assert!(
            fetch_asset(&client, &url, &SMALL, &mut file, &cancel, |_| {}).is_err(),
            "closed listener is a recoverable offline failure"
        );
    }
}
