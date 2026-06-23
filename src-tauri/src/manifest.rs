use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
static DOWNLOAD_TASK_COUNTER: AtomicU64 = AtomicU64::new(1);
static DOWNLOAD_TASKS: OnceLock<DownloadTaskRegistry> = OnceLock::new();

#[cfg(test)]
const PRODUCTION_CSP: &str = "default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; connect-src ipc: http://ipc.localhost";

#[cfg(windows)]
fn windows_no_window_creation_flags() -> u32 {
    CREATE_NO_WINDOW
}

fn new_hidden_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(windows_no_window_creation_flags());
    }
    command
}

#[derive(Debug, Deserialize)]
struct RawManifestRecord {
    path: String,
    object_key: String,
    sha256: String,
    size: u64,
    storage: String,
    updated_at: String,
    visibility: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRecord {
    pub path: String,
    pub object_key: String,
    pub sha256: String,
    pub size: u64,
    pub storage: String,
    pub updated_at: String,
    pub visibility: String,
}

impl From<RawManifestRecord> for ManifestRecord {
    fn from(raw: RawManifestRecord) -> Self {
        Self {
            path: raw.path,
            object_key: raw.object_key,
            sha256: raw.sha256,
            size: raw.size,
            storage: raw.storage,
            updated_at: raw.updated_at,
            visibility: raw.visibility,
        }
    }
}

pub fn parse_manifest_jsonl(input: &str) -> Result<Vec<ManifestRecord>, String> {
    input
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            let raw: RawManifestRecord = serde_json::from_str(line).map_err(|error| {
                format!("Invalid manifest JSON on line {}: {}", index + 1, error)
            })?;
            let record = ManifestRecord::from(raw);
            validate_manifest_record(&record).map_err(|error| {
                format!("Invalid manifest record on line {}: {}", index + 1, error)
            })?;
            Ok(record)
        })
        .collect()
}

fn default_manifest_path(index_repo_path: &str) -> Result<PathBuf, String> {
    let root = resolve_index_repo_path(index_repo_path)?;
    let path = root.join("manifests/files.jsonl");
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("Could not find {}", path.display()))
    }
}

fn load_manifest_from_path(path: &Path) -> Result<Vec<ManifestRecord>, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
    parse_manifest_jsonl(&contents)
}

#[tauri::command]
pub fn load_manifest(index_repo_path: String) -> Result<Vec<ManifestRecord>, String> {
    let path = default_manifest_path(&index_repo_path)?;
    load_manifest_from_path(&path)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequest {
    pub index_repo_path: String,
    pub paths: Vec<String>,
    pub download_root: String,
    pub rclone_path: String,
    pub remote: String,
    pub bucket: String,
    pub download_jobs: u16,
    #[serde(rename = "largeFileThresholdMiB", alias = "largeFileThresholdMib")]
    pub large_file_threshold_mib: u16,
    pub large_file_streams: u16,
    #[serde(default = "default_true")]
    pub show_large_file_progress: bool,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResult {
    pub stdout: String,
    pub stderr: String,
    pub items: Vec<DownloadItemResult>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItemResult {
    pub path: String,
    pub status: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub task_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgressEvent {
    pub task_id: String,
    pub kind: String,
    pub path: Option<String>,
    pub bytes_written: u64,
    pub total_bytes: u64,
    pub completed_files: usize,
    pub failed_files: usize,
    pub total_files: usize,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_index_repo_path")]
    pub index_repo_path: String,
    pub download_root: String,
    pub rclone_path: String,
    pub remote: String,
    pub bucket: String,
    pub download_jobs: u16,
    #[serde(rename = "largeFileThresholdMiB", alias = "largeFileThresholdMib")]
    pub large_file_threshold_mib: u16,
    pub large_file_streams: u16,
    #[serde(default = "default_true")]
    pub show_large_file_progress: bool,
    pub theme: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            index_repo_path: default_index_repo_path(),
            download_root: "downloads/gui".to_string(),
            rclone_path: "rclone".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 4,
            large_file_threshold_mib: 20,
            large_file_streams: 8,
            show_large_file_progress: true,
            theme: "light".to_string(),
        }
    }
}

fn default_index_repo_path() -> String {
    "../TYUT-ebooks-collection-neo".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
}

fn default_settings() -> AppSettings {
    AppSettings::default()
}

#[derive(Default)]
struct DownloadTaskRegistry {
    cancel_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl DownloadTaskRegistry {
    fn insert(&self, task_id: String, cancel_flag: Arc<AtomicBool>) -> Result<(), String> {
        let mut flags = self
            .cancel_flags
            .lock()
            .map_err(|_| "Download task registry lock was poisoned".to_string())?;
        flags.insert(task_id, cancel_flag);
        Ok(())
    }

    fn cancel(&self, task_id: &str) -> Result<(), String> {
        let flags = self
            .cancel_flags
            .lock()
            .map_err(|_| "Download task registry lock was poisoned".to_string())?;
        let flag = flags
            .get(task_id)
            .ok_or_else(|| format!("Download task not found: {}", task_id))?;
        flag.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn remove(&self, task_id: &str) -> Result<(), String> {
        let mut flags = self
            .cancel_flags
            .lock()
            .map_err(|_| "Download task registry lock was poisoned".to_string())?;
        flags.remove(task_id);
        Ok(())
    }
}

fn download_task_registry() -> &'static DownloadTaskRegistry {
    DOWNLOAD_TASKS.get_or_init(DownloadTaskRegistry::default)
}

fn next_download_task_id() -> String {
    let id = DOWNLOAD_TASK_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("download-{}", id)
}

fn download_progress_event(
    task_id: &str,
    kind: &str,
    path: Option<&str>,
    bytes_written: u64,
    total_bytes: u64,
    completed_files: usize,
    failed_files: usize,
    total_files: usize,
    message: &str,
) -> DownloadProgressEvent {
    DownloadProgressEvent {
        task_id: task_id.to_string(),
        kind: kind.to_string(),
        path: path.map(ToString::to_string),
        bytes_written,
        total_bytes,
        completed_files,
        failed_files,
        total_files,
        message: message.to_string(),
    }
}

fn validate_settings(settings: &AppSettings) -> Result<(), String> {
    if settings.index_repo_path.trim().is_empty() {
        return Err("Index repository path is required".to_string());
    }
    if settings.download_root.trim().is_empty() {
        return Err("Download directory is required".to_string());
    }
    validate_rclone_executable(&settings.rclone_path)?;
    validate_remote_name(&settings.remote)?;
    validate_bucket_name(&settings.bucket)?;
    if settings.download_jobs == 0 {
        return Err("Download jobs must be at least 1".to_string());
    }
    if settings.download_jobs > 16 {
        return Err("Download jobs must be between 1 and 16".to_string());
    }
    if settings.large_file_threshold_mib == 0 {
        return Err("Large file threshold must be at least 1 MiB".to_string());
    }
    if settings.large_file_threshold_mib > 4096 {
        return Err("Large file threshold must be between 1 and 4096 MiB".to_string());
    }
    if settings.large_file_streams == 0 {
        return Err("Large file streams must be at least 1".to_string());
    }
    if settings.large_file_streams > 16 {
        return Err("Large file streams must be between 1 and 16".to_string());
    }
    if settings.theme != "light" && settings.theme != "dark" {
        return Err("Theme must be light or dark".to_string());
    }
    Ok(())
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Failed to resolve app config directory: {}", error))?;
    Ok(config_dir.join("settings.json"))
}

fn load_settings_from_path(path: &Path) -> Result<AppSettings, String> {
    if !path.is_file() {
        return Ok(default_settings());
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
    let settings: AppSettings = serde_json::from_str(&contents)
        .map_err(|error| format!("Invalid settings JSON in {}: {}", path.display(), error))?;
    validate_settings(&settings)?;
    Ok(settings)
}

fn save_settings_to_path(path: &Path, settings: &AppSettings) -> Result<(), String> {
    validate_settings(settings)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {}", parent.display(), error))?;
    }
    let contents = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(path, contents)
        .map_err(|error| format!("Failed to write {}: {}", path.display(), error))
}

#[tauri::command]
pub fn load_settings(app: AppHandle) -> Result<AppSettings, String> {
    let path = settings_path(&app)?;
    load_settings_from_path(&path)
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    let path = settings_path(&app)?;
    save_settings_to_path(&path, &settings)?;
    Ok(settings)
}

fn resolve_index_repo_path(index_repo_path: &str) -> Result<PathBuf, String> {
    let current_dir = std::env::current_dir()
        .map_err(|error| format!("Failed to resolve current directory: {}", error))?;
    resolve_index_repo_path_from(index_repo_path, &current_dir)
}

fn resolve_index_repo_path_from(index_repo_path: &str, base_dir: &Path) -> Result<PathBuf, String> {
    let trimmed = index_repo_path.trim();
    if trimmed.is_empty() {
        return Err("Index repository path is required".to_string());
    }

    let configured_path = PathBuf::from(trimmed);
    let is_absolute = configured_path.is_absolute();
    let first_candidate = if is_absolute {
        configured_path.clone()
    } else {
        base_dir.join(&configured_path)
    };

    let root = canonicalize_index_repo_candidate(&first_candidate).or_else(|first_error| {
        if is_absolute || base_dir.file_name().and_then(|name| name.to_str()) != Some("src-tauri") {
            return Err(first_error);
        }

        let Some(project_root) = base_dir.parent() else {
            return Err(first_error);
        };
        let project_root_candidate = project_root.join(&configured_path);
        canonicalize_index_repo_candidate(&project_root_candidate).map_err(|_| first_error)
    })?;

    Ok(root)
}

fn canonicalize_index_repo_candidate(candidate: &Path) -> Result<PathBuf, String> {
    let root = fs::canonicalize(candidate).map_err(|error| {
        format!(
            "Failed to resolve index repository path {}: {}",
            candidate.display(),
            error
        )
    })?;

    if !root.join("manifests/files.jsonl").is_file() {
        return Err(format!(
            "Index repository path {} is missing manifests/files.jsonl",
            root.display()
        ));
    }

    Ok(root)
}

fn validate_download_request(request: &DownloadRequest) -> Result<(), String> {
    if request.paths.is_empty() {
        return Err("Select at least one file before downloading".to_string());
    }
    validate_rclone_executable(&request.rclone_path)?;
    if request.download_jobs == 0 {
        return Err("Download jobs must be at least 1".to_string());
    }
    if request.download_jobs > 16 {
        return Err("Download jobs must be between 1 and 16".to_string());
    }
    if request.large_file_threshold_mib == 0 {
        return Err("Large file threshold must be at least 1 MiB".to_string());
    }
    if request.large_file_threshold_mib > 4096 {
        return Err("Large file threshold must be between 1 and 4096 MiB".to_string());
    }
    if request.large_file_streams == 0 {
        return Err("Large file streams must be at least 1".to_string());
    }
    if request.large_file_streams > 16 {
        return Err("Large file streams must be between 1 and 16".to_string());
    }
    if request.download_root.trim().is_empty() {
        return Err("Download directory is required".to_string());
    }
    validate_remote_name(&request.remote)?;
    validate_bucket_name(&request.bucket)?;
    Ok(())
}

fn validate_rclone_executable(rclone_path: &str) -> Result<(), String> {
    let trimmed = rclone_path.trim();
    if trimmed.is_empty() {
        return Err("rclone path is required".to_string());
    }

    let file_name = Path::new(trimmed)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(trimmed)
        .to_ascii_lowercase();
    if file_name == "rclone" || file_name == "rclone.exe" {
        Ok(())
    } else {
        Err("rclone path must point to rclone or rclone.exe".to_string())
    }
}

fn validate_remote_name(remote: &str) -> Result<(), String> {
    let trimmed = remote.trim().trim_end_matches(':');
    if trimmed.is_empty() {
        return Err("R2 remote is required".to_string());
    }
    if trimmed
        .bytes()
        .all(|byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b'.'))
    {
        Ok(())
    } else {
        Err("R2 remote contains unsupported characters".to_string())
    }
}

fn validate_bucket_name(bucket: &str) -> Result<(), String> {
    let trimmed = bucket.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Err("R2 bucket is required".to_string());
    }
    if trimmed
        .bytes()
        .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.'))
    {
        Ok(())
    } else {
        Err("R2 bucket contains unsupported characters".to_string())
    }
}

fn manifest_path_boundary_error(manifest_path: &str) -> String {
    format!("Manifest path must stay inside the download directory: {manifest_path}")
}

fn validate_manifest_path(manifest_path: &str) -> Result<Vec<&str>, String> {
    if manifest_path.is_empty()
        || manifest_path.contains('\\')
        || manifest_path.contains(':')
        || manifest_path.chars().any(char::is_control)
    {
        return Err(manifest_path_boundary_error(manifest_path));
    }

    let mut segments = Vec::new();
    for segment in manifest_path.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(manifest_path_boundary_error(manifest_path));
        }
        segments.push(segment);
    }

    Ok(segments)
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_manifest_record(record: &ManifestRecord) -> Result<(), String> {
    validate_manifest_path(&record.path)?;

    if record.storage != "r2" || record.visibility != "private" {
        return Err(format!(
            "Manifest record must use private R2 storage: {}",
            record.path
        ));
    }

    if !is_lowercase_sha256(&record.sha256) {
        return Err(format!("Manifest sha256 is invalid for {}", record.path));
    }

    if record.size == 0 && record.sha256 != EMPTY_SHA256 {
        return Err(format!(
            "0-byte manifest record has invalid sha256: {}",
            record.path
        ));
    }

    let object_key = record.object_key.as_str();
    let key_is_plain_relative = !object_key.is_empty()
        && object_key.trim() == object_key
        && !object_key.starts_with('/')
        && !object_key.contains('\\')
        && !object_key.contains(':')
        && !object_key.chars().any(char::is_control);
    if !key_is_plain_relative {
        return Err(format!(
            "Manifest object key does not match sha256 layout for {}",
            record.path
        ));
    }

    let expected_prefix = format!(
        "objects/sha256/{}/{}/{}/",
        &record.sha256[..2],
        &record.sha256[2..4],
        record.sha256
    );
    let Some(file_name) = object_key.strip_prefix(&expected_prefix) else {
        return Err(format!(
            "Manifest object key does not match sha256 layout for {}",
            record.path
        ));
    };

    if file_name.is_empty() || file_name.contains('/') || file_name == "." || file_name == ".." {
        return Err(format!(
            "Manifest object key does not match sha256 layout for {}",
            record.path
        ));
    }

    Ok(())
}

fn build_rclone_cat_args(
    remote: &str,
    bucket: &str,
    record: &ManifestRecord,
) -> Result<Vec<String>, String> {
    let remote_name = remote.trim().trim_end_matches(':');
    let bucket_name = bucket.trim().trim_matches('/');
    validate_manifest_record(record)?;
    validate_remote_name(remote)?;
    validate_bucket_name(bucket)?;

    Ok(vec![
        "cat".to_string(),
        format!("{remote_name}:{bucket_name}/{}", record.object_key),
    ])
}

fn build_rclone_lsf_args(remote: &str, bucket: &str) -> Result<Vec<String>, String> {
    let remote_name = remote.trim().trim_end_matches(':');
    let bucket_name = bucket.trim().trim_matches('/');
    validate_remote_name(remote)?;
    validate_bucket_name(bucket)?;

    Ok(vec![
        "lsf".to_string(),
        format!("{remote_name}:{bucket_name}"),
        "--max-depth".to_string(),
        "1".to_string(),
    ])
}

fn build_rclone_copyto_args(
    remote: &str,
    bucket: &str,
    record: &ManifestRecord,
    temp_path: &Path,
    streams: u16,
    show_progress: bool,
) -> Result<Vec<OsString>, String> {
    let remote_name = remote.trim().trim_end_matches(':');
    let bucket_name = bucket.trim().trim_matches('/');
    validate_manifest_record(record)?;
    validate_remote_name(remote)?;
    validate_bucket_name(bucket)?;
    if streams == 0 || streams > 16 {
        return Err("Large file streams must be between 1 and 16".to_string());
    }

    let mut args = vec![
        OsString::from("copyto"),
        OsString::from(format!("{remote_name}:{bucket_name}/{}", record.object_key)),
        temp_path.as_os_str().to_os_string(),
        OsString::from("--multi-thread-streams"),
        OsString::from(streams.to_string()),
        OsString::from("--multi-thread-cutoff"),
        OsString::from("1M"),
        OsString::from("--multi-thread-chunk-size"),
        OsString::from("16M"),
        OsString::from("--stats"),
        OsString::from("1s"),
    ];
    if show_progress {
        args.push(OsString::from("--progress"));
    }
    Ok(args)
}

fn select_records_by_paths(
    records: &[ManifestRecord],
    paths: &[String],
) -> Result<Vec<ManifestRecord>, String> {
    let by_path: HashMap<&str, &ManifestRecord> = records
        .iter()
        .map(|record| (record.path.as_str(), record))
        .collect();
    let mut selected = Vec::with_capacity(paths.len());

    for path in paths {
        let record = by_path
            .get(path.as_str())
            .ok_or_else(|| format!("Selected path is not present in manifest: {path}"))?;
        selected.push((*record).clone());
    }

    Ok(selected)
}

fn resolve_download_root(index_root: &Path, download_root: &str) -> PathBuf {
    let configured = PathBuf::from(download_root.trim());
    if configured.is_absolute() {
        configured
    } else {
        index_root.join(configured)
    }
}

fn prepare_download_directory(index_root: &Path, download_root: &str) -> Result<PathBuf, String> {
    if download_root.trim().is_empty() {
        return Err("Download directory is required".to_string());
    }

    let directory = resolve_download_root(index_root, download_root);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Failed to create {}: {}", directory.display(), error))?;
    fs::canonicalize(&directory)
        .map_err(|error| format!("Failed to resolve {}: {}", directory.display(), error))
}

fn build_destination_path(
    index_root: &Path,
    download_root: &str,
    manifest_path: &str,
) -> Result<PathBuf, String> {
    let base = resolve_download_root(index_root, download_root);
    let mut destination = base;

    for segment in validate_manifest_path(manifest_path)? {
        destination.push(segment);
    }

    Ok(destination)
}

fn ensure_destination_parent_inside_download_root(
    index_root: &Path,
    download_root: &str,
    destination: &Path,
    manifest_path: &str,
) -> Result<(), String> {
    let base = resolve_download_root(index_root, download_root);
    let base = fs::canonicalize(&base)
        .map_err(|error| format!("Failed to resolve {}: {}", base.display(), error))?;
    let parent = destination
        .parent()
        .ok_or_else(|| manifest_path_boundary_error(manifest_path))?;
    let parent = fs::canonicalize(parent)
        .map_err(|error| format!("Failed to resolve {}: {}", parent.display(), error))?;

    if parent.starts_with(&base) {
        Ok(())
    } else {
        Err(manifest_path_boundary_error(manifest_path))
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn verify_downloaded_file(
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Failed to stat {}: {}", path.display(), error))?;
    if metadata.len() != expected_size {
        return Err(format!(
            "Size mismatch for {}: expected {} bytes, got {} bytes",
            path.display(),
            expected_size,
            metadata.len()
        ));
    }

    let mut file = fs::File::open(path)
        .map_err(|error| format!("Failed to open {}: {}", path.display(), error))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let actual = hex_sha256(&hasher.finalize());
    if actual != expected_sha256.to_ascii_lowercase() {
        return Err(format!(
            "SHA256 mismatch for {}: expected {}, got {}",
            path.display(),
            expected_sha256,
            actual
        ));
    }

    Ok(())
}

fn temp_download_path(destination: &Path) -> PathBuf {
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("download");
    destination.with_file_name(format!("{file_name}.ebook-neo-part"))
}

fn remove_stale_temp_file(temp_path: &Path) -> Result<(), String> {
    if fs::symlink_metadata(temp_path).is_ok() {
        fs::remove_file(temp_path).map_err(|error| {
            format!(
                "Failed to remove stale temp file {}: {}",
                temp_path.display(),
                error
            )
        })?;
    }
    Ok(())
}

fn install_verified_download(temp_path: &Path, destination: &Path) -> Result<(), String> {
    if destination.is_file() {
        fs::remove_file(destination)
            .map_err(|error| format!("Failed to replace {}: {}", destination.display(), error))?;
    }
    fs::rename(temp_path, destination).map_err(|error| {
        format!(
            "Failed to move {} to {}: {}",
            temp_path.display(),
            destination.display(),
            error
        )
    })
}

fn read_pipe_to_string<R>(mut reader: R) -> String
where
    R: Read,
{
    let mut bytes = Vec::new();
    match reader.read_to_end(&mut bytes) {
        Ok(_) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) => format!("Failed to read process output: {error}"),
    }
}

fn parse_rclone_transferred_bytes(line: &str) -> Option<u64> {
    line.split("Transferred:")
        .skip(1)
        .filter_map(|fragment| {
            let before_total = fragment.trim_start().split('/').next()?.trim();
            parse_rclone_size_bytes(before_total)
        })
        .last()
}

fn parse_rclone_size_bytes(value: &str) -> Option<u64> {
    let mut parts = value.split_whitespace();
    let amount = parts.next()?.replace(',', "");
    let unit = parts.next()?;
    let amount: f64 = amount.parse().ok()?;
    let multiplier = match unit {
        "B" => 1_f64,
        "KiB" => 1024_f64,
        "MiB" => 1024_f64 * 1024_f64,
        "GiB" => 1024_f64 * 1024_f64 * 1024_f64,
        "TiB" => 1024_f64 * 1024_f64 * 1024_f64 * 1024_f64,
        "PiB" => 1024_f64 * 1024_f64 * 1024_f64 * 1024_f64 * 1024_f64,
        _ => return None,
    };
    let bytes = amount * multiplier;
    if bytes.is_finite() && bytes >= 0_f64 {
        Some(bytes.round() as u64)
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn stream_rclone_progress_output<R>(
    reader: R,
    enabled: bool,
    progress_sink: &dyn ProgressSink,
    task_id: &str,
    record: &ManifestRecord,
    completed_files: usize,
    failed_files: usize,
    total_files: usize,
) -> String
where
    R: Read,
{
    let mut output = String::new();
    let mut last_progress_bytes = 0_u64;
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                output.push_str(&line);
                if enabled {
                    if let Some(bytes_written) = parse_rclone_transferred_bytes(&line) {
                        let bytes_written = bytes_written.min(record.size);
                        if bytes_written > 0 && bytes_written != last_progress_bytes {
                            last_progress_bytes = bytes_written;
                            progress_sink.emit(download_progress_event(
                                task_id,
                                "progress",
                                Some(&record.path),
                                bytes_written,
                                record.size,
                                completed_files,
                                failed_files,
                                total_files,
                                &format!("copying {}", record.path),
                            ));
                        }
                    }
                }
            }
            Err(error) => {
                output.push_str(&format!("Failed to read rclone progress output: {error}"));
                break;
            }
        }
    }

    output
}

fn stream_download_output<R, W>(
    reader: &mut R,
    writer: &mut W,
    cancel_flag: &AtomicBool,
    progress_sink: &dyn ProgressSink,
    task_id: &str,
    record: &ManifestRecord,
    completed_files: usize,
    failed_files: usize,
    total_files: usize,
) -> Result<u64, String>
where
    R: Read,
    W: Write,
{
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut bytes_written = 0_u64;

    loop {
        if cancel_flag.load(Ordering::SeqCst) {
            return Err("Download canceled".to_string());
        }

        let read = reader.read(&mut buffer).map_err(|error| {
            format!(
                "Failed to read rclone output for {}: {}",
                record.path, error
            )
        })?;
        if read == 0 {
            break;
        }

        writer.write_all(&buffer[..read]).map_err(|error| {
            format!(
                "Failed to write downloaded bytes for {}: {}",
                record.path, error
            )
        })?;
        bytes_written += read as u64;
        progress_sink.emit(download_progress_event(
            task_id,
            "progress",
            Some(&record.path),
            bytes_written,
            record.size,
            completed_files,
            failed_files,
            total_files,
            &format!("streaming {}", record.path),
        ));
    }

    writer.flush().map_err(|error| {
        format!(
            "Failed to flush downloaded bytes for {}: {}",
            record.path, error
        )
    })?;
    Ok(bytes_written)
}

fn download_item_result(path: &str, status: &str, message: String) -> DownloadItemResult {
    DownloadItemResult {
        path: path.to_string(),
        status: status.to_string(),
        message,
    }
}

fn emit_canceled_download_event(
    progress_sink: &dyn ProgressSink,
    task_id: &str,
    record: &ManifestRecord,
    completed_files: usize,
    failed_files: usize,
    total_files: usize,
) {
    progress_sink.emit(download_progress_event(
        task_id,
        "canceled",
        Some(&record.path),
        0,
        record.size,
        completed_files,
        failed_files,
        total_files,
        &format!("canceled {}", record.path),
    ));
}

trait ProgressSink: Sync {
    fn emit(&self, event: DownloadProgressEvent);
}

struct NoopProgressSink;

impl ProgressSink for NoopProgressSink {
    fn emit(&self, _event: DownloadProgressEvent) {}
}

#[cfg(test)]
#[derive(Default)]
struct RecordingProgressSink {
    events: Mutex<Vec<DownloadProgressEvent>>,
}

#[cfg(test)]
impl RecordingProgressSink {
    fn events(&self) -> Vec<DownloadProgressEvent> {
        self.events
            .lock()
            .expect("event lock should not be poisoned")
            .clone()
    }
}

#[cfg(test)]
impl ProgressSink for RecordingProgressSink {
    fn emit(&self, event: DownloadProgressEvent) {
        self.events
            .lock()
            .expect("event lock should not be poisoned")
            .push(event);
    }
}

struct TauriProgressSink {
    app: AppHandle,
}

impl ProgressSink for TauriProgressSink {
    fn emit(&self, event: DownloadProgressEvent) {
        let _ = self.app.emit("download-progress", event);
    }
}

fn try_download_manifest_record_with_prefix_args(
    index_root: &Path,
    request: &DownloadRequest,
    record: &ManifestRecord,
    prefix_args: &[&str],
    task_id: &str,
    progress_sink: &dyn ProgressSink,
    cancel_flag: &AtomicBool,
    completed_files: usize,
    failed_files: usize,
    total_files: usize,
) -> Result<DownloadItemResult, String> {
    validate_manifest_record(record)?;
    let destination = build_destination_path(index_root, &request.download_root, &record.path)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {}", parent.display(), error))?;
    }
    ensure_destination_parent_inside_download_root(
        index_root,
        &request.download_root,
        &destination,
        &record.path,
    )?;

    let temp_path = temp_download_path(&destination);
    remove_stale_temp_file(&temp_path)?;
    if cancel_flag.load(Ordering::SeqCst) {
        let _ = fs::remove_file(&temp_path);
        emit_canceled_download_event(
            progress_sink,
            task_id,
            record,
            completed_files,
            failed_files,
            total_files,
        );
        return Ok(download_item_result(
            &record.path,
            "canceled",
            format!("canceled {}", record.path),
        ));
    }
    progress_sink.emit(download_progress_event(
        task_id,
        "started",
        Some(&record.path),
        0,
        record.size,
        completed_files,
        failed_files,
        total_files,
        &format!("downloading {}", record.path),
    ));
    if record.size == 0 {
        fs::write(&temp_path, [])
            .map_err(|error| format!("Failed to write {}: {}", temp_path.display(), error))?;
        verify_downloaded_file(&temp_path, record.size, &record.sha256)?;
        install_verified_download(&temp_path, &destination)?;
        progress_sink.emit(download_progress_event(
            task_id,
            "finished",
            Some(&record.path),
            0,
            0,
            completed_files + 1,
            failed_files,
            total_files,
            &format!("created empty file {}", record.path),
        ));
        return Ok(download_item_result(
            &record.path,
            "createdEmpty",
            format!("created empty file {}", record.path),
        ));
    }

    let download_result = if record.size >= large_file_threshold_bytes(request) {
        run_rclone_copyto_download(
            request,
            record,
            &temp_path,
            prefix_args,
            cancel_flag,
            progress_sink,
            task_id,
            completed_files,
            failed_files,
            total_files,
        )
    } else {
        run_rclone_cat_download(
            request,
            record,
            &temp_path,
            prefix_args,
            cancel_flag,
            progress_sink,
            task_id,
            completed_files,
            failed_files,
            total_files,
        )
    };
    if let Err(error) = download_result {
        if error == "Download canceled" {
            return Ok(download_item_result(
                &record.path,
                "canceled",
                format!("canceled {}", record.path),
            ));
        }
        return Err(error);
    }

    verify_downloaded_file(&temp_path, record.size, &record.sha256)?;
    install_verified_download(&temp_path, &destination)?;
    progress_sink.emit(download_progress_event(
        task_id,
        "finished",
        Some(&record.path),
        record.size,
        record.size,
        completed_files + 1,
        failed_files,
        total_files,
        &format!("downloaded {}", record.path),
    ));

    Ok(download_item_result(
        &record.path,
        "downloaded",
        format!("downloaded {}", record.path),
    ))
}

fn large_file_threshold_bytes(request: &DownloadRequest) -> u64 {
    u64::from(request.large_file_threshold_mib) * 1024 * 1024
}

#[allow(clippy::too_many_arguments)]
fn run_rclone_cat_download(
    request: &DownloadRequest,
    record: &ManifestRecord,
    temp_path: &Path,
    prefix_args: &[&str],
    cancel_flag: &AtomicBool,
    progress_sink: &dyn ProgressSink,
    task_id: &str,
    completed_files: usize,
    failed_files: usize,
    total_files: usize,
) -> Result<(), String> {
    let args = build_rclone_cat_args(&request.remote, &request.bucket, record)?;

    let mut child = new_hidden_command(&request.rclone_path)
        .args(prefix_args)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to start rclone for {}: {}", record.path, error))?;

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("Failed to capture rclone stdout for {}", record.path))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("Failed to capture rclone stderr for {}", record.path))?;

    let stderr_thread = thread::spawn(move || read_pipe_to_string(stderr));
    let mut temp_file = fs::File::create(temp_path)
        .map_err(|error| format!("Failed to create {}: {}", temp_path.display(), error))?;
    let stream_result = stream_download_output(
        &mut stdout,
        &mut temp_file,
        cancel_flag,
        progress_sink,
        task_id,
        record,
        completed_files,
        failed_files,
        total_files,
    );
    if let Err(error) = stream_result {
        let _ = child.kill();
        let _ = child.wait();
        let stderr_text = stderr_thread
            .join()
            .unwrap_or_else(|_| "Failed to join rclone stderr reader".to_string());
        let _ = fs::remove_file(temp_path);
        if error == "Download canceled" {
            emit_canceled_download_event(
                progress_sink,
                task_id,
                record,
                completed_files,
                failed_files,
                total_files,
            );
            return Err("Download canceled".to_string());
        }
        return Err(format!("{}\n{}", error, stderr_text));
    }
    temp_file
        .flush()
        .map_err(|error| format!("Failed to flush {}: {}", temp_path.display(), error))?;
    drop(temp_file);

    let status = child
        .wait()
        .map_err(|error| format!("Failed to wait for rclone for {}: {}", record.path, error))?;
    let stderr_text = stderr_thread
        .join()
        .unwrap_or_else(|_| "Failed to join rclone stderr reader".to_string());

    if !status.success() {
        let _ = fs::remove_file(temp_path);
        progress_sink.emit(download_progress_event(
            task_id,
            "failed",
            Some(&record.path),
            0,
            record.size,
            completed_files,
            failed_files + 1,
            total_files,
            &format!("failed {}", record.path),
        ));
        return Err(format!(
            "rclone cat failed for {} with status {}.\n{}",
            record.path, status, stderr_text
        ));
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_rclone_copyto_download(
    request: &DownloadRequest,
    record: &ManifestRecord,
    temp_path: &Path,
    prefix_args: &[&str],
    cancel_flag: &AtomicBool,
    progress_sink: &dyn ProgressSink,
    task_id: &str,
    completed_files: usize,
    failed_files: usize,
    total_files: usize,
) -> Result<(), String> {
    let args = build_rclone_copyto_args(
        &request.remote,
        &request.bucket,
        record,
        temp_path,
        request.large_file_streams,
        request.show_large_file_progress,
    )?;

    let mut child = new_hidden_command(&request.rclone_path)
        .args(prefix_args)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to start rclone for {}: {}", record.path, error))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("Failed to capture rclone stdout for {}", record.path))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("Failed to capture rclone stderr for {}", record.path))?;
    let mut stdout = Some(stdout);
    let mut progress_output = String::new();
    let stderr_thread = thread::spawn(move || read_pipe_to_string(stderr));

    thread::scope(|scope| {
        let stdout = stdout
            .take()
            .expect("copyto stdout should be captured before progress reader starts");
        let progress_thread = scope.spawn(|| {
            stream_rclone_progress_output(
                stdout,
                request.show_large_file_progress,
                progress_sink,
                task_id,
                record,
                completed_files,
                failed_files,
                total_files,
            )
        });

        loop {
            if child
                .try_wait()
                .map_err(|error| format!("Failed to poll rclone for {}: {}", record.path, error))?
                .is_some()
            {
                break;
            }
            if cancel_flag.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(temp_path);
                emit_canceled_download_event(
                    progress_sink,
                    task_id,
                    record,
                    completed_files,
                    failed_files,
                    total_files,
                );
                let _ = progress_thread.join();
                return Err("Download canceled".to_string());
            }
            thread::sleep(std::time::Duration::from_millis(100));
        }

        progress_output = progress_thread
            .join()
            .unwrap_or_else(|_| "Failed to join rclone stdout reader".to_string());
        Ok::<_, String>(())
    })?;

    let status = child
        .wait()
        .map_err(|error| format!("Failed to wait for rclone for {}: {}", record.path, error))?;
    let stderr_text = stderr_thread
        .join()
        .unwrap_or_else(|_| "Failed to join rclone stderr reader".to_string());

    if !status.success() {
        let _ = fs::remove_file(temp_path);
        progress_sink.emit(download_progress_event(
            task_id,
            "failed",
            Some(&record.path),
            0,
            record.size,
            completed_files,
            failed_files + 1,
            total_files,
            &format!("failed {}", record.path),
        ));
        return Err(format!(
            "rclone copyto failed for {} with status {}.\n{}\n{}",
            record.path, status, progress_output, stderr_text
        ));
    }

    Ok(())
}

fn download_records(
    index_root: &Path,
    request: &DownloadRequest,
    records: Vec<ManifestRecord>,
) -> Vec<DownloadItemResult> {
    let sink = NoopProgressSink;
    let cancel_flag = AtomicBool::new(false);
    download_records_with_progress(
        index_root,
        request,
        records,
        &[],
        "legacy-download",
        &sink,
        &cancel_flag,
    )
}

#[cfg(test)]
fn download_records_with_prefix_args(
    index_root: &Path,
    request: &DownloadRequest,
    records: Vec<ManifestRecord>,
    prefix_args: &[&str],
) -> Vec<DownloadItemResult> {
    let sink = NoopProgressSink;
    let cancel_flag = AtomicBool::new(false);
    download_records_with_progress(
        index_root,
        request,
        records,
        prefix_args,
        "legacy-download",
        &sink,
        &cancel_flag,
    )
}

fn download_records_with_progress(
    index_root: &Path,
    request: &DownloadRequest,
    records: Vec<ManifestRecord>,
    prefix_args: &[&str],
    task_id: &str,
    progress_sink: &dyn ProgressSink,
    cancel_flag: &AtomicBool,
) -> Vec<DownloadItemResult> {
    let mut deduped = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for record in records {
        if seen.insert(record.path.clone()) {
            deduped.push(record);
        }
    }
    let total_files = deduped.len();
    progress_sink.emit(download_progress_event(
        task_id,
        "queued",
        None,
        0,
        0,
        0,
        0,
        total_files,
        &format!("queued {} file(s)", total_files),
    ));
    for record in &deduped {
        progress_sink.emit(download_progress_event(
            task_id,
            "queued",
            Some(&record.path),
            0,
            record.size,
            0,
            0,
            total_files,
            &format!("queued {}", record.path),
        ));
    }

    if request.download_jobs == 1 || deduped.len() <= 1 {
        let mut completed_files = 0_usize;
        let mut failed_files = 0_usize;
        let mut results = Vec::new();
        for record in &deduped {
            if cancel_flag.load(Ordering::SeqCst) {
                emit_canceled_download_event(
                    progress_sink,
                    task_id,
                    record,
                    completed_files,
                    failed_files,
                    total_files,
                );
                results.push(download_item_result(
                    &record.path,
                    "canceled",
                    format!("canceled {}", record.path),
                ));
                continue;
            }
            let result = match try_download_manifest_record_with_prefix_args(
                index_root,
                request,
                record,
                prefix_args,
                task_id,
                progress_sink,
                cancel_flag,
                completed_files,
                failed_files,
                total_files,
            ) {
                Ok(result) => result,
                Err(error) => download_item_result(&record.path, "failed", error),
            };
            if result.status == "failed" {
                failed_files += 1;
            } else if result.status != "canceled" {
                completed_files += 1;
            }
            results.push(result);
        }
        progress_sink.emit(download_progress_event(
            task_id,
            "completed",
            None,
            0,
            0,
            completed_files,
            failed_files,
            total_files,
            &format!(
                "download task completed: {} complete, {} failed",
                completed_files, failed_files
            ),
        ));
        return results;
    }

    let queue = Arc::new(Mutex::new(VecDeque::from(deduped)));
    let results = Arc::new(Mutex::new(Vec::new()));
    let completed_files = AtomicUsize::new(0);
    let failed_files = AtomicUsize::new(0);
    let worker_count = usize::min(request.download_jobs as usize, total_files);

    thread::scope(|scope| {
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let results = Arc::clone(&results);
            let completed_files = &completed_files;
            let failed_files = &failed_files;

            scope.spawn(move || loop {
                let record = {
                    let mut queue = queue.lock().expect("queue lock should not be poisoned");
                    queue.pop_front()
                };

                let Some(record) = record else {
                    return;
                };

                let result = if cancel_flag.load(Ordering::SeqCst) {
                    emit_canceled_download_event(
                        progress_sink,
                        task_id,
                        &record,
                        completed_files.load(Ordering::SeqCst),
                        failed_files.load(Ordering::SeqCst),
                        total_files,
                    );
                    download_item_result(
                        &record.path,
                        "canceled",
                        format!("canceled {}", record.path),
                    )
                } else {
                    match try_download_manifest_record_with_prefix_args(
                        index_root,
                        request,
                        &record,
                        prefix_args,
                        task_id,
                        progress_sink,
                        cancel_flag,
                        completed_files.load(Ordering::SeqCst),
                        failed_files.load(Ordering::SeqCst),
                        total_files,
                    ) {
                        Ok(result) => result,
                        Err(error) => download_item_result(&record.path, "failed", error),
                    }
                };
                if result.status == "failed" {
                    failed_files.fetch_add(1, Ordering::SeqCst);
                } else if result.status != "canceled" {
                    completed_files.fetch_add(1, Ordering::SeqCst);
                }
                results
                    .lock()
                    .expect("result lock should not be poisoned")
                    .push(result);
            });
        }
    });

    let collected_results = results
        .lock()
        .expect("result lock should not be poisoned")
        .clone();
    progress_sink.emit(download_progress_event(
        task_id,
        "completed",
        None,
        0,
        0,
        completed_files.load(Ordering::SeqCst),
        failed_files.load(Ordering::SeqCst),
        total_files,
        &format!(
            "download task completed: {} complete, {} failed",
            completed_files.load(Ordering::SeqCst),
            failed_files.load(Ordering::SeqCst)
        ),
    ));
    collected_results
}

fn summarize_download_results(results: &[DownloadItemResult]) -> String {
    let succeeded = results
        .iter()
        .filter(|result| result.status != "failed")
        .count();
    let failed = results.len().saturating_sub(succeeded);
    let messages = results
        .iter()
        .map(|result| format!("{}: {}", result.status, result.message))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Downloaded {} file(s), {} failed.\n{}",
        succeeded, failed, messages
    )
}

fn git_update_command_args(index_root: &Path) -> Vec<OsString> {
    vec![
        OsString::from("-C"),
        index_root.as_os_str().to_os_string(),
        OsString::from("pull"),
        OsString::from("--ff-only"),
    ]
}

fn update_manifest_from_git_blocking(index_repo_path: String) -> Result<CommandResult, String> {
    let root = resolve_index_repo_path(&index_repo_path)?;
    let output = new_hidden_command("git")
        .args(git_update_command_args(&root))
        .output()
        .map_err(|error| format!("Failed to start git update command: {}", error))?;

    let result = CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    };

    if output.status.success() {
        Ok(result)
    } else {
        Err(format!(
            "Git update failed with status {}.\n{}\n{}",
            output.status, result.stdout, result.stderr
        ))
    }
}

#[tauri::command]
pub async fn update_manifest_from_git(index_repo_path: String) -> Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || update_manifest_from_git_blocking(index_repo_path))
        .await
        .map_err(|error| format!("Git update worker failed: {}", error))?
}

fn check_rclone_remote_blocking(
    rclone_path: String,
    remote: String,
    bucket: String,
) -> Result<CommandResult, String> {
    validate_rclone_executable(&rclone_path)?;

    let args = build_rclone_lsf_args(&remote, &bucket)?;
    let output = new_hidden_command(rclone_path.trim())
        .args(args)
        .output()
        .map_err(|error| format!("Failed to start rclone check command: {}", error))?;

    let result = CommandResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    };

    if output.status.success() {
        Ok(result)
    } else {
        Err(format!(
            "rclone check failed with status {}.\n{}\n{}",
            output.status, result.stdout, result.stderr
        ))
    }
}

#[tauri::command]
pub async fn check_rclone_remote(
    rclone_path: String,
    remote: String,
    bucket: String,
) -> Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        check_rclone_remote_blocking(rclone_path, remote, bucket)
    })
    .await
    .map_err(|error| format!("rclone check worker failed: {}", error))?
}

#[tauri::command]
pub async fn open_download_root(
    app: AppHandle,
    index_repo_path: String,
    download_root: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = resolve_index_repo_path(&index_repo_path)?;
        let directory = prepare_download_directory(&root, &download_root)?;
        let directory_text = directory.to_string_lossy().into_owned();
        app.opener()
            .open_path(directory_text.clone(), None::<String>)
            .map_err(|error| format!("Failed to open download directory: {}", error))?;
        Ok(directory_text)
    })
    .await
    .map_err(|error| format!("Open download directory worker failed: {}", error))?
}

pub fn download_selected_blocking(request: DownloadRequest) -> Result<DownloadResult, String> {
    let root = resolve_index_repo_path(&request.index_repo_path)?;
    validate_download_request(&request)?;
    let manifest_path = root.join("manifests/files.jsonl");
    let records = load_manifest_from_path(&manifest_path)?;
    let selected = select_records_by_paths(&records, &request.paths)?;
    let items = download_records(&root, &request, selected);

    Ok(DownloadResult {
        stdout: summarize_download_results(&items),
        stderr: String::new(),
        items,
    })
}

#[tauri::command]
pub async fn download_selected(request: DownloadRequest) -> Result<DownloadResult, String> {
    tauri::async_runtime::spawn_blocking(move || download_selected_blocking(request))
        .await
        .map_err(|error| format!("Download worker failed: {}", error))?
}

fn start_download_blocking(
    app: AppHandle,
    request: DownloadRequest,
) -> Result<DownloadTask, String> {
    let root = resolve_index_repo_path(&request.index_repo_path)?;
    validate_download_request(&request)?;
    let manifest_path = root.join("manifests/files.jsonl");
    let records = load_manifest_from_path(&manifest_path)?;
    let selected = select_records_by_paths(&records, &request.paths)?;
    let task_id = next_download_task_id();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    download_task_registry().insert(task_id.clone(), Arc::clone(&cancel_flag))?;

    tauri::async_runtime::spawn_blocking({
        let task_id = task_id.clone();
        move || {
            let sink = TauriProgressSink { app };
            let _items = download_records_with_progress(
                &root,
                &request,
                selected,
                &[],
                &task_id,
                &sink,
                cancel_flag.as_ref(),
            );
            let _ = download_task_registry().remove(&task_id);
        }
    });

    Ok(DownloadTask { task_id })
}

#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    request: DownloadRequest,
) -> Result<DownloadTask, String> {
    tauri::async_runtime::spawn_blocking(move || start_download_blocking(app, request))
        .await
        .map_err(|error| format!("Start download worker failed: {}", error))?
}

#[tauri::command]
pub async fn cancel_download(_app: AppHandle, task_id: String) -> Result<CommandResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        download_task_registry().cancel(&task_id)?;
        Ok(CommandResult {
            stdout: format!("Cancel requested for {}", task_id),
            stderr: String::new(),
        })
    })
    .await
    .map_err(|error| format!("Cancel download worker failed: {}", error))?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object_key_for_sha(sha: &str, file_name: &str) -> String {
        format!(
            "objects/sha256/{}/{}/{}/{}",
            &sha[..2],
            &sha[2..4],
            sha,
            file_name
        )
    }

    #[test]
    fn progress_event_payload_tracks_file_and_batch_counts() {
        let event = download_progress_event(
            "task-1",
            "progress",
            Some("资料/a.txt"),
            2,
            3,
            0,
            0,
            1,
            "streaming 资料/a.txt",
        );

        assert_eq!(event.task_id, "task-1");
        assert_eq!(event.kind, "progress");
        assert_eq!(event.path.as_deref(), Some("资料/a.txt"));
        assert_eq!(event.bytes_written, 2);
        assert_eq!(event.total_bytes, 3);
        assert_eq!(event.completed_files, 0);
        assert_eq!(event.failed_files, 0);
        assert_eq!(event.total_files, 1);
        assert_eq!(event.message, "streaming 资料/a.txt");
    }

    #[test]
    fn parses_rclone_transferred_progress_bytes() {
        assert_eq!(
            parse_rclone_transferred_bytes(
                "Transferred:   \t    1.250 MiB / 128.407 MiB, 1%, 320 KiB/s, ETA 6m46s"
            ),
            Some(1_310_720),
        );
        assert_eq!(
            parse_rclone_transferred_bytes(
                " * 2.exe: 0% / 128.407 MiB, 0 B/s, -Transferred:   18.375 MiB / 128.407 MiB, 14%, 1.875 MiB/s, ETA 58s"
            ),
            Some(19_267_584),
        );
        assert_eq!(
            parse_rclone_transferred_bytes("Transferred:            0 / 1, 0%"),
            None,
        );
    }

    #[test]
    fn download_task_commands_are_async_futures() {
        fn assert_start_download_signature<F, Fut>(_function: F)
        where
            F: Fn(AppHandle, DownloadRequest) -> Fut,
            Fut: std::future::Future<Output = Result<DownloadTask, String>>,
        {
        }

        fn assert_cancel_download_signature<F, Fut>(_function: F)
        where
            F: Fn(AppHandle, String) -> Fut,
            Fut: std::future::Future<Output = Result<CommandResult, String>>,
        {
        }

        assert_start_download_signature(start_download);
        assert_cancel_download_signature(cancel_download);
    }

    #[cfg(windows)]
    fn write_success_fake_rclone(temp_dir: &Path) -> (String, Vec<String>) {
        let fake_rclone = temp_dir.join("fake-rclone.ps1");
        fs::write(
            &fake_rclone,
            "[Console]::OpenStandardOutput().Write([byte[]](97,98,99), 0, 3)\r\n",
        )
        .expect("fake rclone should be written");

        (
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-File".to_string(),
                fake_rclone
                    .to_str()
                    .expect("fake rclone path should be utf-8")
                    .to_string(),
            ],
        )
    }

    #[cfg(not(windows))]
    fn write_success_fake_rclone(temp_dir: &Path) -> (String, Vec<String>) {
        let fake_rclone = temp_dir.join("fake-rclone.sh");
        fs::write(&fake_rclone, "#!/bin/sh\nprintf abc\n").expect("fake rclone should be written");

        (
            "sh".to_string(),
            vec![fake_rclone.to_string_lossy().into_owned()],
        )
    }

    #[cfg(windows)]
    fn write_mixed_result_fake_rclone(temp_dir: &Path) -> (String, Vec<String>) {
        let fake_rclone = temp_dir.join("fake-rclone.ps1");
        fs::write(
            &fake_rclone,
            r#"
$target = $args[-1]
if ($target -like "*good.txt") {
  [Console]::OpenStandardOutput().Write([byte[]](97,98,99), 0, 3)
  exit 0
}
[Console]::Error.Write("missing object")
exit 1
"#,
        )
        .expect("fake rclone should be written");

        (
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-File".to_string(),
                fake_rclone
                    .to_str()
                    .expect("fake rclone path should be utf-8")
                    .to_string(),
            ],
        )
    }

    #[cfg(windows)]
    fn write_copyto_fake_rclone(temp_dir: &Path) -> (String, Vec<String>) {
        let fake_rclone = temp_dir.join("fake-copyto-rclone.ps1");
        let marker = temp_dir.join("copyto.args");
        fs::write(
            &fake_rclone,
            format!(
                r#"
[IO.File]::WriteAllText('{}', ($args -join "`n"), [Text.Encoding]::UTF8)
$destination = $args[2]
$chunk = [byte[]]::new(8192)
for ($i = 0; $i -lt $chunk.Length; $i++) {{ $chunk[$i] = 97 }}
$stream = [IO.File]::Open($destination, [IO.FileMode]::Create, [IO.FileAccess]::Write)
try {{
  for ($i = 0; $i -lt 384; $i++) {{ $stream.Write($chunk, 0, $chunk.Length) }}
}} finally {{
  $stream.Dispose()
}}
"#,
                marker
                    .to_str()
                    .expect("marker path should be utf-8")
                    .replace('\'', "''")
            ),
        )
        .expect("fake copyto rclone should be written");

        (
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-File".to_string(),
                fake_rclone
                    .to_str()
                    .expect("fake rclone path should be utf-8")
                    .to_string(),
            ],
        )
    }

    #[cfg(not(windows))]
    fn write_copyto_fake_rclone(temp_dir: &Path) -> (String, Vec<String>) {
        let fake_rclone = temp_dir.join("fake-copyto-rclone.sh");
        let marker = temp_dir.join("copyto.args");
        fs::write(
            &fake_rclone,
            format!(
                r#"#!/bin/sh
printf '%s\n' "$@" > '{}'
python3 - "$3" <<'PY'
import sys
with open(sys.argv[1], 'wb') as f:
    f.write(b'a' * (3 * 1024 * 1024))
PY
"#,
                marker.to_string_lossy()
            ),
        )
        .expect("fake copyto rclone should be written");

        (
            "sh".to_string(),
            vec![fake_rclone.to_string_lossy().into_owned()],
        )
    }

    #[cfg(windows)]
    fn write_slow_copyto_fake_rclone(temp_dir: &Path) -> (String, Vec<String>) {
        let fake_rclone = temp_dir.join("fake-slow-copyto-rclone.ps1");
        let marker = temp_dir.join("slow-copyto.args");
        fs::write(
            &fake_rclone,
            format!(
                r#"
[IO.File]::WriteAllText('{}', ($args -join "`n"), [Text.Encoding]::UTF8)
$destination = $args[2]
$chunk = [byte[]]::new(8192)
for ($i = 0; $i -lt $chunk.Length; $i++) {{ $chunk[$i] = 97 }}
  [Console]::Out.WriteLine('Transferred:    1 MiB / 3 MiB, 33%, 1 MiB/s, ETA 2s')
  [Console]::Out.Flush()
  Start-Sleep -Milliseconds 900
  [Console]::Out.WriteLine('Transferred:    2 MiB / 3 MiB, 66%, 1 MiB/s, ETA 1s')
  [Console]::Out.Flush()
  $stream = [IO.File]::Open($destination, [IO.FileMode]::Create, [IO.FileAccess]::Write)
  try {{
    for ($i = 0; $i -lt 384; $i++) {{ $stream.Write($chunk, 0, $chunk.Length) }}
  }} finally {{
    $stream.Dispose()
  }}
"#,
                marker
                    .to_str()
                    .expect("marker path should be utf-8")
                    .replace('\'', "''")
            ),
        )
        .expect("fake slow copyto rclone should be written");

        (
            "powershell".to_string(),
            vec![
                "-NoProfile".to_string(),
                "-ExecutionPolicy".to_string(),
                "Bypass".to_string(),
                "-File".to_string(),
                fake_rclone
                    .to_str()
                    .expect("fake rclone path should be utf-8")
                    .to_string(),
            ],
        )
    }

    #[cfg(not(windows))]
    fn write_slow_copyto_fake_rclone(temp_dir: &Path) -> (String, Vec<String>) {
        let fake_rclone = temp_dir.join("fake-slow-copyto-rclone.sh");
        let marker = temp_dir.join("slow-copyto.args");
        fs::write(
            &fake_rclone,
            format!(
                r#"#!/bin/sh
printf '%s\n' "$@" > '{}'
printf '%s\n' 'Transferred:    1 MiB / 3 MiB, 33%, 1 MiB/s, ETA 2s'
sleep 0.9
printf '%s\n' 'Transferred:    2 MiB / 3 MiB, 66%, 1 MiB/s, ETA 1s'
python3 - "$3" <<'PY'
import sys
with open(sys.argv[1], 'wb') as f:
    f.write(b'a' * (3 * 1024 * 1024))
PY
"#,
                marker.to_string_lossy()
            ),
        )
        .expect("fake slow copyto rclone should be written");

        (
            "sh".to_string(),
            vec![fake_rclone.to_string_lossy().into_owned()],
        )
    }

    #[cfg(not(windows))]
    fn write_mixed_result_fake_rclone(temp_dir: &Path) -> (String, Vec<String>) {
        let fake_rclone = temp_dir.join("fake-rclone.sh");
        fs::write(
            &fake_rclone,
            r#"#!/bin/sh
target=""
for arg do
  target="$arg"
done

case "$target" in
  *good.txt)
    printf abc
    exit 0
    ;;
  *)
    printf "missing object" >&2
    exit 1
    ;;
esac
"#,
        )
        .expect("fake rclone should be written");

        (
            "sh".to_string(),
            vec![fake_rclone.to_string_lossy().into_owned()],
        )
    }

    fn test_manifest_record(path: &str, file_name: &str) -> ManifestRecord {
        ManifestRecord {
            path: path.to_string(),
            object_key: object_key_for_sha(
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
                file_name,
            ),
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string(),
            size: 3,
            storage: "r2".to_string(),
            updated_at: "2026-06-12".to_string(),
            visibility: "private".to_string(),
        }
    }

    #[test]
    fn parses_jsonl_manifest_records() {
        let a_sha = "a".repeat(64);
        let b_sha = "b".repeat(64);
        let input = format!(
            r#"{{"path":"资料/a.pdf","object_key":"{}","sha256":"{}","size":123,"storage":"r2","updated_at":"2026-06-12","visibility":"private"}}
{{"path":"课件/b.pptx","object_key":"{}","sha256":"{}","size":456,"storage":"r2","updated_at":"2026-06-12","visibility":"private"}}
"#,
            object_key_for_sha(&a_sha, "a.pdf"),
            a_sha,
            object_key_for_sha(&b_sha, "b.pptx"),
            b_sha
        );

        let records = parse_manifest_jsonl(&input).expect("manifest should parse");

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].path, "资料/a.pdf");
        assert_eq!(records[0].object_key, object_key_for_sha(&a_sha, "a.pdf"));
        assert_eq!(records[1].size, 456);
    }

    #[test]
    fn builds_rclone_cat_args_for_a_manifest_record() {
        let record = ManifestRecord {
            path: "资料/a.pdf".to_string(),
            object_key: object_key_for_sha(&"a".repeat(64), "a.pdf"),
            sha256: "a".repeat(64),
            size: 123,
            storage: "r2".to_string(),
            updated_at: "2026-06-12".to_string(),
            visibility: "private".to_string(),
        };

        let args = build_rclone_cat_args(
            "ebookneo-r2-readonly",
            "tyut-ebooks-collection-neo",
            &record,
        )
        .expect("rclone cat args should build");

        assert_eq!(
            args,
            vec![
                "cat".to_string(),
                format!(
                    "ebookneo-r2-readonly:tyut-ebooks-collection-neo/objects/sha256/aa/aa/{}/a.pdf",
                    "a".repeat(64)
                ),
            ]
        );
    }

    #[test]
    fn builds_rclone_copyto_args_for_large_manifest_record() {
        let record = ManifestRecord {
            path: "资料/a.zip".to_string(),
            object_key: object_key_for_sha(&"a".repeat(64), "a.zip"),
            sha256: "a".repeat(64),
            size: 64 * 1024 * 1024,
            storage: "r2".to_string(),
            updated_at: "2026-06-12".to_string(),
            visibility: "private".to_string(),
        };
        let temp_path = PathBuf::from("E:/Downloads/.ebook-neo-part");

        let args = build_rclone_copyto_args(
            "ebookneo-r2-readonly",
            "tyut-ebooks-collection-neo",
            &record,
            &temp_path,
            8,
            true,
        )
        .expect("rclone copyto args should build");

        assert_eq!(
            args,
            vec![
                OsString::from("copyto"),
                OsString::from(format!(
                    "ebookneo-r2-readonly:tyut-ebooks-collection-neo/objects/sha256/aa/aa/{}/a.zip",
                    "a".repeat(64)
                )),
                temp_path.into_os_string(),
                OsString::from("--multi-thread-streams"),
                OsString::from("8"),
                OsString::from("--multi-thread-cutoff"),
                OsString::from("1M"),
                OsString::from("--multi-thread-chunk-size"),
                OsString::from("16M"),
                OsString::from("--stats"),
                OsString::from("1s"),
                OsString::from("--progress"),
            ]
        );
    }

    #[test]
    fn rejects_manifest_record_with_object_key_outside_sha256_prefix() {
        let record = ManifestRecord {
            path: "资料/a.pdf".to_string(),
            object_key: "other-prefix/a.pdf".to_string(),
            sha256: "a".repeat(64),
            size: 123,
            storage: "r2".to_string(),
            updated_at: "2026-06-12".to_string(),
            visibility: "private".to_string(),
        };

        let error = build_rclone_cat_args(
            "ebookneo-r2-readonly",
            "tyut-ebooks-collection-neo",
            &record,
        )
        .expect_err("unexpected object keys should be rejected");

        assert_eq!(
            error,
            "Manifest object key does not match sha256 layout for 资料/a.pdf"
        );
    }

    #[test]
    fn rejects_manifest_record_with_invalid_sha256() {
        let record = ManifestRecord {
            path: "资料/a.pdf".to_string(),
            object_key: "objects/sha256/aa/aa/not-a-sha/a.pdf".to_string(),
            sha256: "not-a-sha".to_string(),
            size: 123,
            storage: "r2".to_string(),
            updated_at: "2026-06-12".to_string(),
            visibility: "private".to_string(),
        };

        let error = build_rclone_cat_args(
            "ebookneo-r2-readonly",
            "tyut-ebooks-collection-neo",
            &record,
        )
        .expect_err("invalid sha256 should be rejected");

        assert_eq!(error, "Manifest sha256 is invalid for 资料/a.pdf");
    }

    #[test]
    fn rejects_manifest_record_with_unexpected_storage_or_visibility() {
        let record = ManifestRecord {
            path: "资料/a.pdf".to_string(),
            object_key: object_key_for_sha(&"a".repeat(64), "a.pdf"),
            sha256: "a".repeat(64),
            size: 123,
            storage: "local".to_string(),
            updated_at: "2026-06-12".to_string(),
            visibility: "public".to_string(),
        };

        let error = build_rclone_cat_args(
            "ebookneo-r2-readonly",
            "tyut-ebooks-collection-neo",
            &record,
        )
        .expect_err("unexpected storage should be rejected");

        assert_eq!(
            error,
            "Manifest record must use private R2 storage: 资料/a.pdf"
        );
    }

    #[test]
    fn rejects_non_rclone_executables() {
        let error = validate_rclone_executable("powershell")
            .expect_err("only rclone executables should be accepted");

        assert_eq!(error, "rclone path must point to rclone or rclone.exe");
        validate_rclone_executable("E:/Tools/rclone.exe")
            .expect("full rclone.exe paths should be accepted");
    }

    #[test]
    fn rejects_remote_and_bucket_names_with_unsupported_characters() {
        assert_eq!(
            validate_remote_name("ebookneo-r2-readonly;rm")
                .expect_err("remote should reject shell-like punctuation"),
            "R2 remote contains unsupported characters"
        );
        assert_eq!(
            validate_bucket_name("Tyut_Bucket")
                .expect_err("bucket should reject uppercase/underscore"),
            "R2 bucket contains unsupported characters"
        );
    }

    #[test]
    fn builds_read_only_rclone_check_args() {
        let args = build_rclone_lsf_args("ebookneo-r2-readonly:", "/tyut-ebooks-collection-neo/")
            .expect("rclone check args should build");

        assert_eq!(
            args,
            vec![
                "lsf".to_string(),
                "ebookneo-r2-readonly:tyut-ebooks-collection-neo".to_string(),
                "--max-depth".to_string(),
                "1".to_string(),
            ]
        );
    }

    #[test]
    fn selects_manifest_records_by_requested_paths() {
        let records = vec![
            ManifestRecord {
                path: "资料/a.pdf".to_string(),
                object_key: object_key_for_sha(&"a".repeat(64), "a.pdf"),
                sha256: "a".repeat(64),
                size: 123,
                storage: "r2".to_string(),
                updated_at: "2026-06-12".to_string(),
                visibility: "private".to_string(),
            },
            ManifestRecord {
                path: "课件/b.pptx".to_string(),
                object_key: object_key_for_sha(&"b".repeat(64), "b.pptx"),
                sha256: "b".repeat(64),
                size: 456,
                storage: "r2".to_string(),
                updated_at: "2026-06-12".to_string(),
                visibility: "private".to_string(),
            },
        ];

        let selected = select_records_by_paths(&records, &["课件/b.pptx".to_string()])
            .expect("selection should find requested record");

        assert_eq!(selected, vec![records[1].clone()]);
    }

    #[test]
    fn rejects_missing_selected_manifest_path() {
        let records = vec![ManifestRecord {
            path: "资料/a.pdf".to_string(),
            object_key: object_key_for_sha(&"a".repeat(64), "a.pdf"),
            sha256: "a".repeat(64),
            size: 123,
            storage: "r2".to_string(),
            updated_at: "2026-06-12".to_string(),
            visibility: "private".to_string(),
        }];

        let error = select_records_by_paths(&records, &["资料/missing.pdf".to_string()])
            .expect_err("missing selection should fail");

        assert_eq!(
            error,
            "Selected path is not present in manifest: 资料/missing.pdf"
        );
    }

    #[test]
    fn builds_safe_destination_paths_under_download_root() {
        let root = PathBuf::from("E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo");
        let path = build_destination_path(&root, "downloads/gui", "资料/数据结构/a.pdf")
            .expect("destination should be safe");

        assert_eq!(
            path,
            PathBuf::from("E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo")
                .join("downloads/gui")
                .join("资料/数据结构/a.pdf")
        );
    }

    #[test]
    fn prepares_download_directory_under_index_root() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-download-dir-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(temp_dir.join("manifests")).expect("manifests dir should be created");
        fs::write(temp_dir.join("manifests/files.jsonl"), "").expect("manifest should be created");

        let prepared = prepare_download_directory(&temp_dir, "downloads/gui")
            .expect("directory should be prepared");

        assert!(prepared.is_absolute());
        assert!(prepared.is_dir());
        assert_eq!(
            prepared,
            fs::canonicalize(temp_dir.join("downloads/gui"))
                .expect("download dir should canonicalize")
        );

        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn rejects_manifest_paths_that_escape_download_root() {
        let root = PathBuf::from("E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo");
        let error = build_destination_path(&root, "downloads/gui", "../secret.pdf")
            .expect_err("parent traversal should fail");

        assert_eq!(
            error,
            "Manifest path must stay inside the download directory: ../secret.pdf"
        );
    }

    #[test]
    #[cfg(unix)]
    fn rejects_manifest_paths_that_escape_download_root_through_symlink() {
        use std::os::unix::fs::symlink;

        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-symlink-download-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        let index_root = temp_dir.join("index");
        let outside = temp_dir.join("outside");
        fs::create_dir_all(index_root.join("downloads")).expect("download dir should be created");
        fs::create_dir_all(&outside).expect("outside dir should be created");
        symlink(&outside, index_root.join("downloads/link")).expect("symlink should be created");

        let destination = build_destination_path(&index_root, "downloads", "link/secret.txt")
            .expect("path construction should stay lexical");
        let error = ensure_destination_parent_inside_download_root(
            &index_root,
            "downloads",
            &destination,
            "link/secret.txt",
        )
        .expect_err("symlink escape should be rejected");

        assert_eq!(
            error,
            "Manifest path must stay inside the download directory: link/secret.txt"
        );
        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn rejects_manifest_paths_with_windows_absolute_prefixes() {
        let root = PathBuf::from("E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo");
        let error = build_destination_path(&root, "downloads/gui", "C:/Windows/win.ini")
            .expect_err("Windows absolute-style manifest paths should fail");

        assert_eq!(
            error,
            "Manifest path must stay inside the download directory: C:/Windows/win.ini"
        );
    }

    #[test]
    fn verifies_downloaded_file_size_and_sha256() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-verify-download-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let file_path = temp_dir.join("abc.txt");
        fs::write(&file_path, b"abc").expect("test file should be written");

        verify_downloaded_file(
            &file_path,
            3,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
        .expect("file should verify");

        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn verifies_downloaded_file_on_small_stack() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-small-stack-verify-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let file_path = temp_dir.join("abc.txt");
        fs::write(&file_path, b"abc").expect("test file should be written");

        let file_path_for_thread = file_path.clone();
        let verify_result = thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || {
                verify_downloaded_file(
                    &file_path_for_thread,
                    3,
                    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
                )
            })
            .expect("verification thread should spawn")
            .join()
            .expect("verification should not overflow the stack");

        verify_result.expect("file should verify");
        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn download_records_emits_streaming_progress_events() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-progress-download-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let (fake_rclone_command, fake_rclone_prefix_args) = write_success_fake_rclone(&temp_dir);
        let request = DownloadRequest {
            index_repo_path: temp_dir.to_string_lossy().into_owned(),
            paths: vec!["资料/a.txt".to_string()],
            download_root: temp_dir.join("downloads").to_string_lossy().into_owned(),
            rclone_path: fake_rclone_command,
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 1,
            large_file_threshold_mib: 20,
            large_file_streams: 8,
            show_large_file_progress: true,
        };
        let sink = RecordingProgressSink::default();
        let cancel_flag = AtomicBool::new(false);

        let results = download_records_with_progress(
            &temp_dir,
            &request,
            vec![test_manifest_record("资料/a.txt", "a.txt")],
            &fake_rclone_prefix_args
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            "task-progress",
            &sink,
            &cancel_flag,
        );
        let events = sink.events();

        assert_eq!(results[0].status, "downloaded", "{}", results[0].message);
        assert!(events.iter().any(|event| event.kind == "queued"));
        assert!(events.iter().any(|event| event.kind == "started"));
        assert!(events
            .iter()
            .any(|event| event.kind == "progress" && event.bytes_written == 3));
        assert!(events.iter().any(|event| {
            event.kind == "finished" && event.completed_files == 1 && event.total_files == 1
        }));
        assert!(events.iter().any(|event| event.kind == "completed"));
        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn canceled_download_removes_partial_file_and_reports_canceled() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-cancel-download-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let request = DownloadRequest {
            index_repo_path: temp_dir.to_string_lossy().into_owned(),
            paths: vec!["资料/a.txt".to_string()],
            download_root: temp_dir.join("downloads").to_string_lossy().into_owned(),
            rclone_path: "rclone".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 1,
            large_file_threshold_mib: 20,
            large_file_streams: 8,
            show_large_file_progress: true,
        };
        let record = test_manifest_record("资料/a.txt", "a.txt");
        let destination = temp_dir.join("downloads").join("资料/a.txt");
        fs::create_dir_all(
            destination
                .parent()
                .expect("destination should have parent"),
        )
        .expect("download parent should be created");
        let temp_download = temp_download_path(&destination);
        fs::write(&temp_download, b"ab").expect("partial file should be written");
        let sink = RecordingProgressSink::default();
        let cancel_flag = AtomicBool::new(true);

        let result = try_download_manifest_record_with_prefix_args(
            &temp_dir,
            &request,
            &record,
            &[],
            "task-cancel",
            &sink,
            &cancel_flag,
            0,
            0,
            1,
        )
        .expect("canceled download should produce an item result");

        assert_eq!(result.status, "canceled");
        assert!(
            !temp_download.exists(),
            "partial file should be removed on cancellation"
        );
        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn queued_cancellation_emits_canceled_progress_events() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-queued-cancel-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let request = DownloadRequest {
            index_repo_path: temp_dir.to_string_lossy().into_owned(),
            paths: vec!["资料/a.txt".to_string(), "资料/b.txt".to_string()],
            download_root: temp_dir.join("downloads").to_string_lossy().into_owned(),
            rclone_path: "rclone".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 1,
            large_file_threshold_mib: 20,
            large_file_streams: 8,
            show_large_file_progress: true,
        };
        let sink = RecordingProgressSink::default();
        let cancel_flag = AtomicBool::new(true);

        let results = download_records_with_progress(
            &temp_dir,
            &request,
            vec![
                test_manifest_record("资料/a.txt", "a.txt"),
                test_manifest_record("资料/b.txt", "b.txt"),
            ],
            &[],
            "task-queued-cancel",
            &sink,
            &cancel_flag,
        );
        let events = sink.events();
        let canceled_events = events
            .iter()
            .filter(|event| event.kind == "canceled")
            .collect::<Vec<_>>();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|result| result.status == "canceled"));
        assert_eq!(canceled_events.len(), 2);
        assert_eq!(canceled_events[0].path.as_deref(), Some("资料/a.txt"));
        assert_eq!(canceled_events[1].path.as_deref(), Some("资料/b.txt"));
        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn download_records_emits_queued_events_for_each_selected_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-queued-events-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let request = DownloadRequest {
            index_repo_path: temp_dir.to_string_lossy().into_owned(),
            paths: vec!["资料/a.txt".to_string(), "资料/b.txt".to_string()],
            download_root: temp_dir.join("downloads").to_string_lossy().into_owned(),
            rclone_path: "rclone".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 1,
            large_file_threshold_mib: 20,
            large_file_streams: 8,
            show_large_file_progress: true,
        };
        let sink = RecordingProgressSink::default();
        let cancel_flag = AtomicBool::new(true);

        let _results = download_records_with_progress(
            &temp_dir,
            &request,
            vec![
                test_manifest_record("资料/a.txt", "a.txt"),
                test_manifest_record("资料/b.txt", "b.txt"),
            ],
            &[],
            "task-queued",
            &sink,
            &cancel_flag,
        );
        let queued_paths = sink
            .events()
            .iter()
            .filter(|event| event.kind == "queued")
            .filter_map(|event| event.path.clone())
            .collect::<Vec<_>>();

        assert_eq!(queued_paths, vec!["资料/a.txt", "资料/b.txt"]);
        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn download_records_streams_rclone_output_and_verifies_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-direct-download-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let (fake_rclone_command, fake_rclone_prefix_args) = write_success_fake_rclone(&temp_dir);

        let request = DownloadRequest {
            index_repo_path: temp_dir.to_string_lossy().into_owned(),
            paths: vec!["资料/a.txt".to_string()],
            download_root: temp_dir.join("downloads").to_string_lossy().into_owned(),
            rclone_path: fake_rclone_command,
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 1,
            large_file_threshold_mib: 20,
            large_file_streams: 8,
            show_large_file_progress: true,
        };
        let record = ManifestRecord {
            path: "资料/a.txt".to_string(),
            object_key: object_key_for_sha(
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
                "a.txt",
            ),
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_string(),
            size: 3,
            storage: "r2".to_string(),
            updated_at: "2026-06-12".to_string(),
            visibility: "private".to_string(),
        };

        let results = download_records_with_prefix_args(
            &temp_dir,
            &request,
            vec![record],
            &fake_rclone_prefix_args
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        );
        let downloaded = temp_dir.join("downloads").join("资料/a.txt");
        let temp_downloaded = temp_download_path(&downloaded);

        assert_eq!(
            results,
            vec![DownloadItemResult {
                path: "资料/a.txt".to_string(),
                status: "downloaded".to_string(),
                message: "downloaded 资料/a.txt".to_string(),
            }]
        );
        assert_eq!(
            fs::read(downloaded).expect("downloaded file should exist"),
            b"abc"
        );
        assert!(
            !temp_downloaded.exists(),
            "temporary download file should be moved into place"
        );
        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn large_file_download_uses_rclone_copyto_and_installs_verified_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-copyto-download-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let (fake_rclone_command, fake_rclone_prefix_args) = write_copyto_fake_rclone(&temp_dir);
        let large_bytes = vec![b'a'; 3 * 1024 * 1024];
        let large_sha256 = hex_sha256(&Sha256::digest(&large_bytes));

        let request = DownloadRequest {
            index_repo_path: temp_dir.to_string_lossy().into_owned(),
            paths: vec!["资料/large.txt".to_string()],
            download_root: temp_dir.join("downloads").to_string_lossy().into_owned(),
            rclone_path: fake_rclone_command,
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 1,
            large_file_threshold_mib: 1,
            large_file_streams: 8,
            show_large_file_progress: true,
        };
        let record = ManifestRecord {
            path: "资料/large.txt".to_string(),
            object_key: object_key_for_sha(&large_sha256, "large.txt"),
            sha256: large_sha256,
            size: 3 * 1024 * 1024,
            storage: "r2".to_string(),
            updated_at: "2026-06-12".to_string(),
            visibility: "private".to_string(),
        };

        let results = download_records_with_prefix_args(
            &temp_dir,
            &request,
            vec![record],
            &fake_rclone_prefix_args
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        );
        let downloaded = temp_dir.join("downloads").join("资料/large.txt");
        let temp_downloaded = temp_download_path(&downloaded);
        let marker = temp_dir.join("copyto.args");

        assert_eq!(results[0].status, "downloaded", "{}", results[0].message);
        assert_eq!(
            fs::read(&downloaded).expect("downloaded file should exist"),
            large_bytes
        );
        assert!(
            !temp_downloaded.exists(),
            "temporary download file should be moved into place"
        );
        assert!(
            fs::read_to_string(marker)
                .expect("fake rclone marker should exist")
                .contains("copyto"),
            "large download should use rclone copyto"
        );
        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn large_file_progress_emits_temp_file_progress_when_enabled() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-copyto-progress-enabled-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let (fake_rclone_command, fake_rclone_prefix_args) =
            write_slow_copyto_fake_rclone(&temp_dir);
        let large_bytes = vec![b'a'; 3 * 1024 * 1024];
        let large_sha256 = hex_sha256(&Sha256::digest(&large_bytes));
        let request = DownloadRequest {
            index_repo_path: temp_dir.to_string_lossy().into_owned(),
            paths: vec!["资料/large.txt".to_string()],
            download_root: temp_dir.join("downloads").to_string_lossy().into_owned(),
            rclone_path: fake_rclone_command,
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 1,
            large_file_threshold_mib: 1,
            large_file_streams: 8,
            show_large_file_progress: true,
        };
        let record = ManifestRecord {
            path: "资料/large.txt".to_string(),
            object_key: object_key_for_sha(&large_sha256, "large.txt"),
            sha256: large_sha256,
            size: 3 * 1024 * 1024,
            storage: "r2".to_string(),
            updated_at: "2026-06-12".to_string(),
            visibility: "private".to_string(),
        };
        let sink = RecordingProgressSink::default();
        let cancel_flag = AtomicBool::new(false);

        let results = download_records_with_progress(
            &temp_dir,
            &request,
            vec![record],
            &fake_rclone_prefix_args
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            "task-copyto-progress-enabled",
            &sink,
            &cancel_flag,
        );
        let events = sink.events();

        assert_eq!(results[0].status, "downloaded", "{}", results[0].message);
        assert!(events.iter().any(|event| {
            event.kind == "progress"
                && event.path.as_deref() == Some("资料/large.txt")
                && event.bytes_written > 0
                && event.bytes_written < 3 * 1024 * 1024
        }));
        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn large_file_progress_skips_temp_file_progress_when_disabled() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-copyto-progress-disabled-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let (fake_rclone_command, fake_rclone_prefix_args) =
            write_slow_copyto_fake_rclone(&temp_dir);
        let large_bytes = vec![b'a'; 3 * 1024 * 1024];
        let large_sha256 = hex_sha256(&Sha256::digest(&large_bytes));
        let request = DownloadRequest {
            index_repo_path: temp_dir.to_string_lossy().into_owned(),
            paths: vec!["资料/large.txt".to_string()],
            download_root: temp_dir.join("downloads").to_string_lossy().into_owned(),
            rclone_path: fake_rclone_command,
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 1,
            large_file_threshold_mib: 1,
            large_file_streams: 8,
            show_large_file_progress: false,
        };
        let record = ManifestRecord {
            path: "资料/large.txt".to_string(),
            object_key: object_key_for_sha(&large_sha256, "large.txt"),
            sha256: large_sha256,
            size: 3 * 1024 * 1024,
            storage: "r2".to_string(),
            updated_at: "2026-06-12".to_string(),
            visibility: "private".to_string(),
        };
        let sink = RecordingProgressSink::default();
        let cancel_flag = AtomicBool::new(false);

        let results = download_records_with_progress(
            &temp_dir,
            &request,
            vec![record],
            &fake_rclone_prefix_args
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            "task-copyto-progress-disabled",
            &sink,
            &cancel_flag,
        );
        let events = sink.events();

        assert_eq!(results[0].status, "downloaded", "{}", results[0].message);
        assert!(!events.iter().any(|event| {
            event.kind == "progress" && event.path.as_deref() == Some("资料/large.txt")
        }));
        assert!(events.iter().any(|event| {
            event.kind == "finished"
                && event.path.as_deref() == Some("资料/large.txt")
                && event.bytes_written == 3 * 1024 * 1024
        }));
        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn download_records_reports_failures_without_stopping_the_batch() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-structured-download-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let (fake_rclone_command, fake_rclone_prefix_args) =
            write_mixed_result_fake_rclone(&temp_dir);

        let request = DownloadRequest {
            index_repo_path: temp_dir.to_string_lossy().into_owned(),
            paths: vec!["资料/good.txt".to_string(), "资料/bad.txt".to_string()],
            download_root: temp_dir.join("downloads").to_string_lossy().into_owned(),
            rclone_path: fake_rclone_command,
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 1,
            large_file_threshold_mib: 20,
            large_file_streams: 8,
            show_large_file_progress: true,
        };
        let records = vec![
            ManifestRecord {
                path: "资料/good.txt".to_string(),
                object_key: object_key_for_sha(
                    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
                    "good.txt",
                ),
                sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
                    .to_string(),
                size: 3,
                storage: "r2".to_string(),
                updated_at: "2026-06-12".to_string(),
                visibility: "private".to_string(),
            },
            ManifestRecord {
                path: "资料/bad.txt".to_string(),
                object_key: object_key_for_sha(&"a".repeat(64), "bad.txt"),
                sha256: "a".repeat(64),
                size: 3,
                storage: "r2".to_string(),
                updated_at: "2026-06-12".to_string(),
                visibility: "private".to_string(),
            },
        ];

        let results = download_records_with_prefix_args(
            &temp_dir,
            &request,
            records,
            &fake_rclone_prefix_args
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        );

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "资料/good.txt");
        assert_eq!(results[0].status, "downloaded");
        assert_eq!(results[1].path, "资料/bad.txt");
        assert_eq!(results[1].status, "failed");
        assert!(results[1].message.contains("missing object"));

        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn download_request_validation_does_not_require_python() {
        let request = DownloadRequest {
            index_repo_path: "../TYUT-ebooks-collection-neo".to_string(),
            paths: vec!["资料/a.pdf".to_string(), "课件/b.pptx".to_string()],
            download_root: "downloads/gui".to_string(),
            rclone_path: "rclone".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 4,
            large_file_threshold_mib: 20,
            large_file_streams: 8,
            show_large_file_progress: true,
        };

        validate_download_request(&request).expect("download request should validate");
    }

    #[test]
    fn async_download_command_uses_blocking_worker() {
        fn assert_download_future<F>(future: F) -> F
        where
            F: std::future::Future<Output = Result<DownloadResult, String>>,
        {
            future
        }

        let request = DownloadRequest {
            index_repo_path: "../TYUT-ebooks-collection-neo".to_string(),
            paths: vec!["资料/a.pdf".to_string()],
            download_root: "downloads/gui".to_string(),
            rclone_path: "rclone".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 1,
            large_file_threshold_mib: 20,
            large_file_streams: 8,
            show_large_file_progress: true,
        };

        let future = assert_download_future(download_selected(request));

        let _ = future;
    }

    #[test]
    fn async_update_manifest_command_uses_blocking_worker() {
        fn assert_command_future<F>(future: F) -> F
        where
            F: std::future::Future<Output = Result<CommandResult, String>>,
        {
            future
        }

        let future = assert_command_future(update_manifest_from_git(
            "../TYUT-ebooks-collection-neo".to_string(),
        ));

        let _ = future;
    }

    #[test]
    fn async_rclone_check_command_uses_blocking_worker() {
        fn assert_command_future<F>(future: F) -> F
        where
            F: std::future::Future<Output = Result<CommandResult, String>>,
        {
            future
        }

        let future = assert_command_future(check_rclone_remote(
            "rclone".to_string(),
            "ebookneo-r2-readonly".to_string(),
            "tyut-ebooks-collection-neo".to_string(),
        ));

        let _ = future;
    }

    #[test]
    fn opener_capability_allows_open_path() {
        let capability_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("capabilities/default.json");
        let contents =
            fs::read_to_string(capability_path).expect("default capability should be readable");

        assert!(
            !contents.contains("opener:allow-open-path"),
            "frontend opener permissions should stay disabled; open the prepared download root through a Rust command"
        );
    }

    #[test]
    fn tauri_config_enables_a_production_csp() {
        let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
        let contents = fs::read_to_string(config_path).expect("tauri config should be readable");
        let config: serde_json::Value =
            serde_json::from_str(&contents).expect("tauri config should parse");
        let csp = config["app"]["security"]["csp"]
            .as_str()
            .expect("production CSP should be configured as a string");

        assert_eq!(csp, PRODUCTION_CSP);
        assert!(!csp.contains("unsafe-eval"));
    }

    #[test]
    fn tauri_config_uses_kyanetwork_identity_chinese_installers_and_legacy_nsis_upgrade_template() {
        let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
        let contents = fs::read_to_string(config_path).expect("tauri config should be readable");
        let config: serde_json::Value =
            serde_json::from_str(&contents).expect("tauri config should parse");

        assert_eq!(config["identifier"], "work.kyanet.ebookneo");
        assert_eq!(config["bundle"]["publisher"], "Kyanetwork");
        assert_eq!(
            config["bundle"]["windows"]["nsis"]["template"],
            "windows/nsis/installer.nsi"
        );
        assert_eq!(
            config["bundle"]["windows"]["nsis"]["languages"],
            serde_json::json!(["SimpChinese", "English"])
        );
        assert_eq!(
            config["bundle"]["windows"]["wix"]["language"],
            serde_json::json!(["zh-CN", "en-US"])
        );

        let template_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("windows/nsis/installer.nsi");
        let template_contents =
            fs::read_to_string(template_path).expect("custom NSIS template should be readable");
        assert!(template_contents.contains("LEGACYMANUPRODUCTKEY_TYUTEBOOKS"));
        assert!(template_contents
            .contains("ReadRegStr $4 SHCTX \"${LEGACYMANUPRODUCTKEY_TYUTEBOOKS}\" \"\""));
        assert!(template_contents.contains("${If} $4 != \"\""));
        assert!(template_contents.contains("StrCpy $R1 \"$R1 _?=$4\""));
    }

    #[test]
    #[cfg(windows)]
    fn windows_child_processes_use_no_window_creation_flag() {
        assert_eq!(windows_no_window_creation_flags(), 0x08000000);
    }

    #[test]
    fn rejects_empty_download_selection() {
        let request = DownloadRequest {
            index_repo_path: "../TYUT-ebooks-collection-neo".to_string(),
            paths: Vec::new(),
            download_root: "downloads/gui".to_string(),
            rclone_path: "rclone".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 4,
            large_file_threshold_mib: 20,
            large_file_streams: 8,
            show_large_file_progress: true,
        };

        let error = validate_download_request(&request).expect_err("empty selection should fail");

        assert_eq!(error, "Select at least one file before downloading");
    }

    #[test]
    fn rejects_download_jobs_above_limit() {
        let request = DownloadRequest {
            index_repo_path: "../TYUT-ebooks-collection-neo".to_string(),
            paths: vec!["资料/a.pdf".to_string()],
            download_root: "downloads/gui".to_string(),
            rclone_path: "rclone".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 17,
            large_file_threshold_mib: 20,
            large_file_streams: 8,
            show_large_file_progress: true,
        };

        let error = validate_download_request(&request).expect_err("too many jobs should fail");

        assert_eq!(error, "Download jobs must be between 1 and 16");
    }

    #[test]
    fn default_settings_include_download_jobs_and_light_theme() {
        let settings = default_settings();

        assert_eq!(settings.index_repo_path, "../TYUT-ebooks-collection-neo");
        assert_eq!(settings.download_root, "downloads/gui");
        assert_eq!(settings.rclone_path, "rclone");
        assert_eq!(settings.remote, "ebookneo-r2-readonly");
        assert_eq!(settings.bucket, "tyut-ebooks-collection-neo");
        assert_eq!(settings.download_jobs, 4);
        assert_eq!(settings.large_file_threshold_mib, 20);
        assert_eq!(settings.large_file_streams, 8);
        assert!(settings.show_large_file_progress);
        assert_eq!(settings.theme, "light");
    }

    #[test]
    fn download_request_accepts_frontend_mib_field_spelling() {
        let request: DownloadRequest = serde_json::from_value(serde_json::json!({
            "indexRepoPath": "../TYUT-ebooks-collection-neo",
            "paths": ["资料/a.pdf"],
            "downloadRoot": "downloads/gui",
            "rclonePath": "rclone",
            "remote": "ebookneo-r2-readonly",
            "bucket": "tyut-ebooks-collection-neo",
            "downloadJobs": 4,
            "largeFileThresholdMiB": 20,
            "largeFileStreams": 8,
            "showLargeFileProgress": true
        }))
        .expect("frontend request spelling should deserialize");

        assert_eq!(request.large_file_threshold_mib, 20);
        assert_eq!(request.large_file_streams, 8);
        assert!(request.show_large_file_progress);
    }

    #[test]
    fn app_settings_serializes_frontend_mib_field_spelling_and_reads_legacy_mib() {
        let value = serde_json::to_value(default_settings()).expect("settings should serialize");
        assert_eq!(
            value
                .get("largeFileThresholdMiB")
                .and_then(|item| item.as_u64()),
            Some(20),
        );
        assert!(value.get("largeFileThresholdMib").is_none());
        assert_eq!(
            value
                .get("showLargeFileProgress")
                .and_then(|item| item.as_bool()),
            Some(true),
        );

        let mut legacy_value = value;
        let legacy_object = legacy_value
            .as_object_mut()
            .expect("serialized settings should be an object");
        legacy_object.remove("largeFileThresholdMiB");
        legacy_object.insert("largeFileThresholdMib".to_string(), serde_json::json!(32));

        let settings: AppSettings = serde_json::from_value(legacy_value)
            .expect("legacy settings spelling should deserialize");
        assert_eq!(settings.large_file_threshold_mib, 32);
    }

    #[test]
    fn settings_roundtrip_through_json_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-settings-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let settings_path = temp_dir.join("settings.json");
        let settings = AppSettings {
            index_repo_path: "E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo".to_string(),
            download_root: "E:/Downloads/TYUT".to_string(),
            rclone_path: "E:/Tools/rclone.exe".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 6,
            large_file_threshold_mib: 32,
            large_file_streams: 12,
            show_large_file_progress: false,
            theme: "dark".to_string(),
        };

        save_settings_to_path(&settings_path, &settings).expect("settings should save");
        let loaded = load_settings_from_path(&settings_path).expect("settings should load");

        assert_eq!(loaded, settings);
        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }

    #[test]
    fn missing_settings_file_loads_defaults() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-missing-settings-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        let settings_path = temp_dir.join("settings.json");

        let loaded = load_settings_from_path(&settings_path).expect("defaults should load");

        assert_eq!(loaded, default_settings());
    }

    #[test]
    fn builds_git_update_command_args() {
        let root = PathBuf::from("E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo");
        assert_eq!(
            git_update_command_args(&root),
            vec![
                OsString::from("-C"),
                root.as_os_str().to_os_string(),
                OsString::from("pull"),
                OsString::from("--ff-only")
            ]
        );
    }

    #[test]
    fn validates_index_repository_path_shape() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-index-repo-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(temp_dir.join("manifests")).expect("manifests dir should be created");
        fs::write(temp_dir.join("manifests/files.jsonl"), "").expect("manifest should be created");

        let expected = fs::canonicalize(&temp_dir).expect("temp path should canonicalize");
        let resolved =
            resolve_index_repo_path(temp_dir.to_str().expect("temp path should be utf-8"))
                .expect("index repo path should resolve");

        assert_eq!(resolved, expected);
        fs::remove_dir_all(&resolved).expect("temp dir should be removed");
    }

    #[test]
    fn resolves_default_index_repository_relative_to_project_root_from_src_tauri() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-index-repo-src-tauri-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        let app_root = temp_dir.join("ebook-neo-desktop");
        let src_tauri_dir = app_root.join("src-tauri");
        let index_root = temp_dir.join("TYUT-ebooks-collection-neo");
        fs::create_dir_all(&src_tauri_dir).expect("src-tauri dir should be created");
        fs::create_dir_all(index_root.join("manifests")).expect("manifests dir should be created");
        fs::write(index_root.join("manifests/files.jsonl"), "")
            .expect("manifest should be created");

        let expected = fs::canonicalize(&index_root).expect("index root should canonicalize");
        let resolved =
            resolve_index_repo_path_from("../TYUT-ebooks-collection-neo", &src_tauri_dir)
                .expect("index repo path should resolve from project root");

        assert_eq!(resolved, expected);
        fs::remove_dir_all(&temp_dir).expect("temp dir should be removed");
    }
}
