// ModelManager — the catalog of local ASR models plus their on-disk state.
// PROJECT_SPEC.md §6.1; modeled on Handy's manager (managers/model.rs) but kept
// minimal and Audie-style: no specta, no score bars / translation / language
// fields, and (this phase) no downloader — that lands in Phase 2.
//
// The catalog is DATA, not source: it lives in models_catalog.toml and is read at
// init (CLAUDE.md prompt-as-data; even Handy regrets hardcoding its registry).
// On init we scan models_dir so any GGML file already on disk is usable with zero
// clicks: catalog entries flip is_downloaded when their file is present, and stray
// *.bin files not in the catalog are picked up as is_custom (Handy's
// discover_custom_whisper_models pattern).

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};

/// One local ASR model the user can select. Exposed to the frontend, so it has a
/// hand-written Zod mirror in src/types/settings.ts (Audie hand-writes Zod, no
/// specta). Keep fields and their serde names in sync with that schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub filename: String,
    /// Download URL (catalog models). None for custom on-disk models.
    pub url: Option<String>,
    /// Expected SHA256. None = verification skipped (honest TODO this phase).
    pub sha256: Option<String>,
    pub size_mb: u64,
    pub is_downloaded: bool,
    /// In-flight download (drives the picker's progress/cancel row in Phase 3).
    pub is_downloading: bool,
    /// Bytes already on disk in the `.partial` file — lets the UI show resume
    /// progress for a paused/cancelled download. 0 when none.
    pub partial_size: u64,
    pub is_recommended: bool,
    pub is_custom: bool,
    /// Inference engine. whisper only for now.
    pub engine: String,
}

/// Payload for the `model-download-progress` event. Hand-written Zod mirror in
/// src/types/settings.ts (DownloadProgressSchema). Mirrors Handy's event shape so
/// the frontend store can listen with the same field names.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub model_id: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
}

/// RAII guard that resets `is_downloading` and drops the cancel flag for a model
/// on every error/early-return path of `download_model`. Disarmed on success,
/// where the completion branch does its own state update (also setting
/// is_downloaded). Mirrors Handy's DownloadCleanup — without it, a `?` mid-download
/// would leave the model wedged in the "downloading" state forever.
struct DownloadCleanup<'a> {
    available_models: &'a Mutex<HashMap<String, ModelInfo>>,
    cancel_flags: &'a Mutex<HashMap<String, Arc<AtomicBool>>>,
    model_id: String,
    disarmed: bool,
}

impl Drop for DownloadCleanup<'_> {
    fn drop(&mut self) {
        if self.disarmed {
            return;
        }
        if let Ok(mut models) = self.available_models.lock() {
            if let Some(model) = models.get_mut(self.model_id.as_str()) {
                model.is_downloading = false;
            }
        }
        if let Ok(mut flags) = self.cancel_flags.lock() {
            flags.remove(&self.model_id);
        }
    }
}

/// One `[[models]]` entry as written in models_catalog.toml. The on-disk-state
/// fields (is_downloaded, is_custom) are not in the file — they're computed by the
/// scan — so the catalog row is a narrower struct that we lift into ModelInfo.
#[derive(Debug, Deserialize)]
struct CatalogEntry {
    id: String,
    name: String,
    description: String,
    filename: String,
    url: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    size_mb: u64,
    #[serde(default)]
    is_recommended: bool,
    engine: String,
}

#[derive(Debug, Deserialize)]
struct Catalog {
    models: Vec<CatalogEntry>,
}

/// The bundled catalog source, parsed once at init. `include_str!` keeps it in the
/// binary so there's no runtime file to ship/resolve.
const CATALOG_TOML: &str = include_str!("../../models_catalog.toml");

pub struct ModelManager {
    app_handle: AppHandle,
    models_dir: PathBuf,
    /// id → model. Catalog entries plus any stray on-disk custom models.
    available_models: Mutex<HashMap<String, ModelInfo>>,
    /// id → cancel flag for the in-flight download of that model. Set true to ask
    /// the chunk loop to stop (keeping the `.partial` file for later resume).
    cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl ModelManager {
    /// Build the manager: create models_dir, parse the catalog, then scan the dir
    /// to mark downloaded catalog models and pick up custom on-disk *.bin files.
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let models_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|err| anyhow::anyhow!("resolve app data dir: {err}"))?
            .join("models");

        if !models_dir.exists() {
            fs::create_dir_all(&models_dir)?;
        }

        let mut available_models = parse_catalog(CATALOG_TOML)?;
        scan_models_dir(&models_dir, &mut available_models);

        Ok(Self {
            app_handle: app_handle.clone(),
            models_dir,
            available_models: Mutex::new(available_models),
            cancel_flags: Mutex::new(HashMap::new()),
        })
    }

    /// Degraded fallback when `new` can't resolve the data dir: an empty registry so
    /// the app still launches (the local-ASR picker just shows nothing). models_dir
    /// is a best-effort relative path that simply won't match any file.
    pub fn empty(app_handle: &AppHandle) -> Self {
        Self {
            app_handle: app_handle.clone(),
            models_dir: PathBuf::from("models"),
            available_models: Mutex::new(HashMap::new()),
            cancel_flags: Mutex::new(HashMap::new()),
        }
    }

    /// All known models (catalog + discovered custom), with current on-disk state.
    /// Not `unwrap`: a poisoned lock is unrecoverable here, so surface it instead.
    pub fn get_available_models(&self) -> Result<Vec<ModelInfo>> {
        let models = self
            .available_models
            .lock()
            .map_err(|_| anyhow::anyhow!("model registry lock poisoned"))?;
        Ok(models.values().cloned().collect())
    }

    /// Resolve a downloaded model id to its absolute file path, or None if the id
    /// is unknown or its file isn't present. Used by transcription to turn the
    /// selected catalog id into a path for whisper.cpp.
    pub fn downloaded_model_path(&self, model_id: &str) -> Option<PathBuf> {
        let models = self.available_models.lock().ok()?;
        let model = models.get(model_id)?;
        if !model.is_downloaded {
            return None;
        }
        let path = self.models_dir.join(&model.filename);
        path.exists().then_some(path)
    }

    /// True when the given id exists in the registry (catalog or custom). Lets the
    /// command layer keep the persisted selection honest.
    pub fn has_model(&self, model_id: &str) -> bool {
        self.available_models
            .lock()
            .map(|models| models.contains_key(model_id))
            .unwrap_or(false)
    }

    /// Download a catalog model to `models_dir`. Streams chunk-by-chunk into a
    /// `{filename}.partial`, resuming from any existing partial via a Range request
    /// (with 200-vs-206 fallback when the server ignores ranges, like Handy). On
    /// completion: size check, optional SHA256 verify, then atomic rename to the
    /// final filename. Cancel is cooperative — the cancel flag is checked each chunk
    /// and leaves the partial in place for a later resume. A RAII guard resets
    /// `is_downloading` on every error path. Custom on-disk models have no URL and
    /// can't be (re-)downloaded.
    pub async fn download_model(&self, model_id: &str) -> Result<()> {
        let model_info = {
            let models = self.lock_models()?;
            models.get(model_id).cloned()
        };
        let model_info = model_info.ok_or_else(|| anyhow::anyhow!("未知的本地模型：{model_id}"))?;

        let url = model_info.url.clone().ok_or_else(|| {
            anyhow::anyhow!("模型 {model_id} 没有下载地址（本地自定义模型不可下载）")
        })?;
        let model_path = self.models_dir.join(&model_info.filename);
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", model_info.filename));

        // Already fully downloaded — clean up any stale partial and return.
        if model_path.exists() {
            if partial_path.exists() {
                let _ = fs::remove_file(&partial_path);
            }
            self.mark_downloaded(model_id)?;
            return Ok(());
        }

        // Resume point: how many bytes the partial already holds (0 = fresh start).
        let mut resume_from = if partial_path.exists() {
            let size = partial_path.metadata()?.len();
            info!("resuming download of {model_id} from byte {size}");
            size
        } else {
            info!("starting download of {model_id} from {url}");
            0
        };

        // Mark downloading + register a cancel flag for this download.
        let cancel_flag = Arc::new(AtomicBool::new(false));
        {
            let mut models = self.lock_models()?;
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = true;
            }
        }
        {
            let mut flags = self.lock_flags()?;
            flags.insert(model_id.to_string(), cancel_flag.clone());
        }

        // Guard resets is_downloading + drops the cancel flag on any `?` below.
        // Disarmed only on the success path (which additionally sets is_downloaded).
        let mut cleanup = DownloadCleanup {
            available_models: &self.available_models,
            cancel_flags: &self.cancel_flags,
            model_id: model_id.to_string(),
            disarmed: false,
        };

        let client = reqwest::Client::new();
        let mut request = client.get(&url);
        if resume_from > 0 {
            request = request.header("Range", format!("bytes={resume_from}-"));
        }
        let mut response = request.send().await?;

        // Tried to resume but got 200 (not 206): the server ignored the Range, so
        // the body is the whole file. Appending it to the partial would corrupt the
        // result — drop the partial and restart fresh (Handy's fallback).
        if resume_from > 0 && response.status() == reqwest::StatusCode::OK {
            warn!("server ignored Range for {model_id}, restarting download fresh");
            drop(response);
            let _ = fs::remove_file(&partial_path);
            resume_from = 0;
            response = client.get(&url).send().await?;
        }

        if !response.status().is_success()
            && response.status() != reqwest::StatusCode::PARTIAL_CONTENT
        {
            return Err(anyhow::anyhow!(
                "下载失败：HTTP {} ({model_id})",
                response.status()
            ));
        }

        // For a resumed (206) response content_length is the *remaining* bytes, so
        // add the resume point back to get the true total.
        let total_size = resume_total(resume_from, response.content_length());

        let mut file = if resume_from > 0 {
            fs::OpenOptions::new().append(true).open(&partial_path)?
        } else {
            File::create(&partial_path)?
        };

        let mut downloaded = resume_from;
        self.emit_progress(model_id, downloaded, total_size);

        // Throttle progress events to ~10/sec so the UI isn't flooded.
        let mut last_emit = Instant::now();
        let throttle = Duration::from_millis(100);

        // `Response::chunk()` pulls the body incrementally without the reqwest
        // `stream` feature, letting us check the cancel flag between chunks.
        while let Some(chunk) = response.chunk().await? {
            if cancel_flag.load(Ordering::Relaxed) {
                drop(file);
                info!("download cancelled for {model_id} (partial kept for resume)");
                // Guard handles is_downloading + cancel-flag cleanup on drop.
                return Ok(());
            }

            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;

            if last_emit.elapsed() >= throttle {
                self.emit_progress(model_id, downloaded, total_size);
                last_emit = Instant::now();
            }
        }

        // Final 100% tick so the UI doesn't stall just under complete.
        self.emit_progress(model_id, downloaded, total_size);

        file.flush()?;
        drop(file); // close before size check / verify / rename

        // Size check first: a truncated body (dropped connection) must not be
        // promoted to the final file. Delete the partial so the next try restarts.
        if total_size > 0 {
            let actual = partial_path.metadata()?.len();
            if actual != total_size {
                let _ = fs::remove_file(&partial_path);
                return Err(anyhow::anyhow!(
                    "下载不完整：期望 {total_size} 字节，实际 {actual} 字节（{model_id}）"
                ));
            }
        }

        // SHA256 verify only when the catalog carries an expected hash (skip when
        // None — no fabricated hashes this phase). Hashing a multi-hundred-MB file
        // is CPU-bound, so it runs in spawn_blocking off the async executor. On
        // mismatch the partial is deleted so the next attempt starts fresh.
        if let Some(expected) = model_info.sha256.clone() {
            let verify_path = partial_path.clone();
            let result = tokio::task::spawn_blocking(move || compute_sha256(&verify_path))
                .await
                .map_err(|err| anyhow::anyhow!("sha256 task panicked: {err}"))?;
            match result {
                Ok(actual) if actual.eq_ignore_ascii_case(&expected) => {}
                Ok(actual) => {
                    warn!("sha256 mismatch for {model_id}: expected {expected}, got {actual}");
                    let _ = fs::remove_file(&partial_path);
                    return Err(anyhow::anyhow!(
                        "下载校验失败：文件已损坏，请重试（{model_id}）"
                    ));
                }
                Err(err) => {
                    let _ = fs::remove_file(&partial_path);
                    return Err(anyhow::anyhow!("校验下载失败：{err}（{model_id}）"));
                }
            }
        }

        // Atomic promotion of the verified partial to its final name.
        fs::rename(&partial_path, &model_path)?;

        // Success: do the state update here (sets is_downloaded too) and disarm the
        // guard so it doesn't undo it.
        cleanup.disarmed = true;
        {
            let mut models = self.lock_models()?;
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = false;
                model.is_downloaded = true;
                model.partial_size = 0;
            }
        }
        if let Ok(mut flags) = self.cancel_flags.lock() {
            flags.remove(model_id);
        }

        let _ = self.app_handle.emit("model-download-complete", model_id);
        info!("downloaded {model_id} -> {}", model_path.display());
        Ok(())
    }

    /// Ask the in-flight download for `model_id` to stop. Cooperative: sets the
    /// cancel flag (the chunk loop notices and exits, keeping the `.partial` for
    /// resume) and flips is_downloading immediately for UI responsiveness.
    pub fn cancel_download(&self, model_id: &str) -> Result<()> {
        {
            let flags = self.lock_flags()?;
            if let Some(flag) = flags.get(model_id) {
                flag.store(true, Ordering::Relaxed);
                info!("cancel flag set for {model_id}");
            } else {
                warn!("cancel_download: no active download for {model_id}");
            }
        }
        {
            let mut models = self.lock_models()?;
            if let Some(model) = models.get_mut(model_id) {
                model.is_downloading = false;
            }
        }
        let _ = self.app_handle.emit("model-download-cancelled", model_id);
        Ok(())
    }

    /// Delete a model's files (final + any `.partial`). Catalog models flip back to
    /// not-downloaded; custom on-disk models are removed from the registry entirely
    /// (no URL means they can't be re-downloaded). Emits `model-deleted`.
    pub fn delete_model(&self, model_id: &str) -> Result<()> {
        let model_info = {
            let models = self.lock_models()?;
            models.get(model_id).cloned()
        };
        let model_info = model_info.ok_or_else(|| anyhow::anyhow!("未知的本地模型：{model_id}"))?;

        let model_path = self.models_dir.join(&model_info.filename);
        let partial_path = self
            .models_dir
            .join(format!("{}.partial", model_info.filename));

        let mut deleted = false;
        if model_path.exists() {
            fs::remove_file(&model_path)?;
            deleted = true;
        }
        if partial_path.exists() {
            fs::remove_file(&partial_path)?;
            deleted = true;
        }
        if !deleted {
            return Err(anyhow::anyhow!("没有可删除的模型文件：{model_id}"));
        }

        {
            let mut models = self.lock_models()?;
            if model_info.is_custom {
                models.remove(model_id);
            } else if let Some(model) = models.get_mut(model_id) {
                model.is_downloaded = false;
                model.partial_size = 0;
            }
        }

        let _ = self.app_handle.emit("model-deleted", model_id);
        Ok(())
    }

    /// Emit a throttled-by-caller `model-download-progress` tick. Event emit
    /// failures are non-fatal (the download proceeds regardless).
    fn emit_progress(&self, model_id: &str, downloaded: u64, total: u64) {
        let progress = DownloadProgress {
            model_id: model_id.to_string(),
            downloaded,
            total,
            percentage: progress_percentage(downloaded, total),
        };
        let _ = self.app_handle.emit("model-download-progress", &progress);
    }

    /// Flip a catalog model to downloaded (used when the final file already exists).
    fn mark_downloaded(&self, model_id: &str) -> Result<()> {
        let mut models = self.lock_models()?;
        if let Some(model) = models.get_mut(model_id) {
            model.is_downloaded = true;
            model.is_downloading = false;
            model.partial_size = 0;
        }
        Ok(())
    }

    fn lock_models(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, ModelInfo>>> {
        self.available_models
            .lock()
            .map_err(|_| anyhow::anyhow!("model registry lock poisoned"))
    }

    fn lock_flags(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, Arc<AtomicBool>>>> {
        self.cancel_flags
            .lock()
            .map_err(|_| anyhow::anyhow!("cancel-flags lock poisoned"))
    }
}

/// True total download size. On a fresh download (`resume_from == 0`) it's the
/// server-reported Content-Length; on a resume (206) Content-Length covers only the
/// remaining bytes, so the already-downloaded `resume_from` is added back. An absent
/// Content-Length yields 0 (treated as "unknown" by the percentage math).
fn resume_total(resume_from: u64, content_length: Option<u64>) -> u64 {
    resume_from + content_length.unwrap_or(0)
}

/// Download percentage in [0, 100]. 0.0 when total is unknown (0) to avoid div-by-zero.
fn progress_percentage(downloaded: u64, total: u64) -> f64 {
    if total > 0 {
        (downloaded as f64 / total as f64) * 100.0
    } else {
        0.0
    }
}

/// SHA256 hex digest of a file, read in 64KB chunks to bound memory on large models.
fn compute_sha256(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Parse the catalog TOML into the id→ModelInfo map. Catalog rows start with
/// is_downloaded=false / is_custom=false; the scan fixes is_downloaded.
fn parse_catalog(toml_str: &str) -> Result<HashMap<String, ModelInfo>> {
    let catalog: Catalog = toml::from_str(toml_str)?;
    let mut models = HashMap::new();
    for entry in catalog.models {
        models.insert(
            entry.id.clone(),
            ModelInfo {
                id: entry.id,
                name: entry.name,
                description: entry.description,
                filename: entry.filename,
                url: entry.url,
                sha256: entry.sha256,
                size_mb: entry.size_mb,
                is_downloaded: false,
                is_downloading: false,
                partial_size: 0,
                is_recommended: entry.is_recommended,
                is_custom: false,
                engine: entry.engine,
            },
        );
    }
    Ok(models)
}

/// Scan models_dir: flip is_downloaded for catalog entries whose file is present,
/// and add any stray *.bin not already in the catalog as a custom model
/// (zero-click "any on-disk model usable"). Mirrors Handy's
/// discover_custom_whisper_models. A missing dir / unreadable entry is tolerated.
fn scan_models_dir(models_dir: &Path, models: &mut HashMap<String, ModelInfo>) {
    // Mark catalog models present on disk.
    for model in models.values_mut() {
        model.is_downloaded = models_dir.join(&model.filename).exists();
    }

    if !models_dir.exists() {
        return;
    }

    // Filenames already claimed by the catalog — never re-add these as custom.
    let catalog_filenames: std::collections::HashSet<String> =
        models.values().map(|m| m.filename.clone()).collect();

    let entries = match fs::read_dir(models_dir) {
        Ok(entries) => entries,
        Err(err) => {
            warn!("scan models dir {}: {err}", models_dir.display());
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(filename) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        // Skip hidden files and .partial downloads; only GGML *.bin are usable.
        if filename.starts_with('.') || !filename.ends_with(".bin") {
            continue;
        }
        if catalog_filenames.contains(filename) {
            continue;
        }

        let model_id = filename.trim_end_matches(".bin").to_string();
        if models.contains_key(&model_id) {
            continue;
        }

        let size_mb = path
            .metadata()
            .map(|meta| meta.len() / (1024 * 1024))
            .unwrap_or(0);

        info!("discovered custom whisper model: {model_id} ({filename}, {size_mb} MB)");
        models.insert(
            model_id.clone(),
            ModelInfo {
                id: model_id,
                name: humanize_filename(filename),
                description: "用户提供的本地模型".to_string(),
                filename: filename.to_string(),
                url: None,    // custom models have no download URL
                sha256: None, // and skip verification
                size_mb,
                is_downloaded: true, // already on disk
                is_downloading: false,
                partial_size: 0,
                is_recommended: false,
                is_custom: true,
                engine: "whisper".to_string(),
            },
        );
    }
}

/// Turn a model filename into a readable display name: drop .bin, split on - / _,
/// capitalize each word. "whisper_medical_v2.bin" → "Whisper Medical V2".
fn humanize_filename(filename: &str) -> String {
    filename
        .trim_end_matches(".bin")
        .replace(['-', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Create a unique temp dir under the system temp dir (no `tempfile` dep —
    /// Phase 1 adds no new crate). Caller removes it.
    fn unique_temp_dir() -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("audie-model-test-{}-{}", std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(dir: &Path, name: &str, contents: &[u8]) {
        fs::write(dir.join(name), contents).unwrap();
    }

    #[test]
    fn catalog_parses_with_recommended_and_real_urls() {
        let models = parse_catalog(CATALOG_TOML).expect("bundled catalog must parse");
        assert!(!models.is_empty(), "catalog should not be empty");

        // small is the recommended entry; all entries start not-downloaded.
        let small = models.get("small").expect("small entry present");
        assert!(small.is_recommended);
        assert!(!small.is_downloaded);
        assert!(!small.is_custom);
        assert_eq!(small.engine, "whisper");
        assert_eq!(small.filename, "ggml-small.bin");
        assert!(small
            .url
            .as_deref()
            .unwrap()
            .starts_with("https://huggingface.co/ggerganov/whisper.cpp/"));

        // exactly one recommended (keeps the picker default unambiguous).
        let recommended = models.values().filter(|m| m.is_recommended).count();
        assert_eq!(recommended, 1);
    }

    #[test]
    fn scan_marks_catalog_model_downloaded_when_file_present() {
        let dir = unique_temp_dir();
        // ggml-small.bin present, ggml-base.bin absent.
        write_file(&dir, "ggml-small.bin", b"fake ggml");

        let mut models = parse_catalog(CATALOG_TOML).unwrap();
        scan_models_dir(&dir, &mut models);

        assert!(models.get("small").unwrap().is_downloaded);
        assert!(!models.get("base").unwrap().is_downloaded);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn scan_picks_up_stray_bin_as_custom() {
        let dir = unique_temp_dir();
        write_file(&dir, "my-custom-model.bin", b"data");
        write_file(&dir, "whisper_medical_v2.bin", b"more");
        // ignored: hidden, non-.bin, .partial, a directory, a catalog filename.
        write_file(&dir, ".hidden.bin", b"x");
        write_file(&dir, "notes.txt", b"x");
        write_file(&dir, "ggml-base.bin.partial", b"x");
        write_file(&dir, "ggml-tiny.bin", b"x"); // catalog filename → marks tiny, not custom
        fs::create_dir(dir.join("subdir.bin")).unwrap();

        let mut models = parse_catalog(CATALOG_TOML).unwrap();
        scan_models_dir(&dir, &mut models);

        let custom = models.get("my-custom-model").expect("stray .bin picked up");
        assert!(custom.is_custom);
        assert!(custom.is_downloaded);
        assert!(custom.url.is_none());
        assert!(custom.sha256.is_none());
        assert_eq!(custom.name, "My Custom Model");

        // underscore handling + capitalization.
        assert_eq!(
            models.get("whisper_medical_v2").unwrap().name,
            "Whisper Medical V2"
        );

        // catalog file marks the catalog entry, not a custom one.
        assert!(models.get("tiny").unwrap().is_downloaded);
        assert!(!models.contains_key("ggml-tiny"));

        // ignored entries never become models.
        assert!(!models.contains_key(".hidden"));
        assert!(!models.contains_key("notes"));
        assert!(!models.contains_key("ggml-base.bin")); // .partial
        assert!(!models.contains_key("subdir"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn scan_nonexistent_dir_leaves_catalog_not_downloaded() {
        let mut models = parse_catalog(CATALOG_TOML).unwrap();
        scan_models_dir(Path::new("/no/such/audie/models/dir"), &mut models);
        assert!(models.values().all(|m| !m.is_downloaded && !m.is_custom));
    }

    #[test]
    fn humanize_filename_capitalizes_words() {
        assert_eq!(humanize_filename("my-custom-model.bin"), "My Custom Model");
        assert_eq!(
            humanize_filename("whisper_medical_v2.bin"),
            "Whisper Medical V2"
        );
    }

    #[test]
    fn resume_total_adds_resume_point_for_206() {
        // Fresh download: total == server Content-Length.
        assert_eq!(resume_total(0, Some(1000)), 1000);
        // Resume: server reports only the remaining bytes; we add back what's on disk.
        assert_eq!(resume_total(400, Some(600)), 1000);
        // Missing Content-Length is treated as unknown (0) plus the resume point.
        assert_eq!(resume_total(0, None), 0);
        assert_eq!(resume_total(400, None), 400);
    }

    #[test]
    fn progress_percentage_handles_unknown_total() {
        assert_eq!(progress_percentage(0, 0), 0.0); // unknown total: no div-by-zero
        assert_eq!(progress_percentage(500, 1000), 50.0);
        assert_eq!(progress_percentage(1000, 1000), 100.0);
        // Resume math feeds through: 400 already + 200 more of a 1000-byte file.
        assert_eq!(progress_percentage(600, resume_total(400, Some(600))), 60.0);
    }

    #[test]
    fn compute_sha256_matches_known_digest() {
        let dir = unique_temp_dir();
        // SHA256("abc") is a well-known fixed vector.
        write_file(&dir, "f.bin", b"abc");
        let digest = compute_sha256(&dir.join("f.bin")).unwrap();
        assert_eq!(
            digest,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn cancel_cleanup_guard_resets_downloading_and_drops_flag() {
        // The RAII guard is what keeps a model from being wedged "downloading" after
        // an error/cancel. Exercise it directly (no network, no AppHandle).
        let mut models = parse_catalog(CATALOG_TOML).unwrap();
        if let Some(m) = models.get_mut("small") {
            m.is_downloading = true;
        }
        let models = Mutex::new(models);
        let flags: Mutex<HashMap<String, Arc<AtomicBool>>> = Mutex::new(HashMap::new());
        flags
            .lock()
            .unwrap()
            .insert("small".to_string(), Arc::new(AtomicBool::new(true)));

        {
            let _guard = DownloadCleanup {
                available_models: &models,
                cancel_flags: &flags,
                model_id: "small".to_string(),
                disarmed: false,
            };
        } // drop here

        assert!(!models.lock().unwrap().get("small").unwrap().is_downloading);
        assert!(!flags.lock().unwrap().contains_key("small"));
    }

    #[test]
    fn disarmed_guard_leaves_state_untouched() {
        // On the success path the guard is disarmed so it must not undo the
        // is_downloading flag the success branch is about to clear itself.
        let mut models = parse_catalog(CATALOG_TOML).unwrap();
        if let Some(m) = models.get_mut("small") {
            m.is_downloading = true;
        }
        let models = Mutex::new(models);
        let flags: Mutex<HashMap<String, Arc<AtomicBool>>> = Mutex::new(HashMap::new());

        {
            let mut guard = DownloadCleanup {
                available_models: &models,
                cancel_flags: &flags,
                model_id: "small".to_string(),
                disarmed: false,
            };
            guard.disarmed = true;
        }

        assert!(models.lock().unwrap().get("small").unwrap().is_downloading);
    }
}
