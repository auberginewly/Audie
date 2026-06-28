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
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Result;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

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
    pub is_recommended: bool,
    pub is_custom: bool,
    /// Inference engine. whisper only for now.
    pub engine: String,
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

// Used by the manager for the command-facing access path; referenced from
// lib.rs / commands.rs once those land. Keep the AppHandle so future phases
// (download progress events) can emit without changing the constructor.
impl ModelManager {
    #[allow(dead_code)] // emitter handle reserved for Phase 2 download events.
    pub(crate) fn app_handle(&self) -> &AppHandle {
        &self.app_handle
    }
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
}
