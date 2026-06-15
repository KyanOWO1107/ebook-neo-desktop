use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Manager};

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
            Ok(ManifestRecord::from(raw))
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
            theme: "light".to_string(),
        }
    }
}

fn default_index_repo_path() -> String {
    "../TYUT-ebooks-collection-neo".to_string()
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

fn validate_settings(settings: &AppSettings) -> Result<(), String> {
    if settings.index_repo_path.trim().is_empty() {
        return Err("Index repository path is required".to_string());
    }
    if settings.download_root.trim().is_empty() {
        return Err("Download directory is required".to_string());
    }
    if settings.rclone_path.trim().is_empty() {
        return Err("rclone path is required".to_string());
    }
    if settings.remote.trim().is_empty() {
        return Err("R2 remote is required".to_string());
    }
    if settings.bucket.trim().is_empty() {
        return Err("R2 bucket is required".to_string());
    }
    if settings.download_jobs == 0 {
        return Err("Download jobs must be at least 1".to_string());
    }
    if settings.download_jobs > 16 {
        return Err("Download jobs must be between 1 and 16".to_string());
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
    if request.rclone_path.trim().is_empty() {
        return Err("rclone path is required".to_string());
    }
    if request.download_jobs == 0 {
        return Err("Download jobs must be at least 1".to_string());
    }
    if request.download_jobs > 16 {
        return Err("Download jobs must be between 1 and 16".to_string());
    }
    if request.download_root.trim().is_empty() {
        return Err("Download directory is required".to_string());
    }
    if request.remote.trim().is_empty() {
        return Err("R2 remote is required".to_string());
    }
    if request.bucket.trim().is_empty() {
        return Err("R2 bucket is required".to_string());
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
    let object_key = record.object_key.trim().trim_start_matches('/');

    if remote_name.is_empty() {
        return Err("R2 remote is required".to_string());
    }
    if bucket_name.is_empty() {
        return Err("R2 bucket is required".to_string());
    }
    if object_key.is_empty() {
        return Err(format!(
            "Manifest record has no object key: {}",
            record.path
        ));
    }

    Ok(vec![
        "cat".to_string(),
        format!("{remote_name}:{bucket_name}/{object_key}"),
    ])
}

fn build_rclone_lsf_args(remote: &str, bucket: &str) -> Result<Vec<String>, String> {
    let remote_name = remote.trim().trim_end_matches(':');
    let bucket_name = bucket.trim().trim_matches('/');

    if remote_name.is_empty() {
        return Err("R2 remote is required".to_string());
    }
    if bucket_name.is_empty() {
        return Err("R2 bucket is required".to_string());
    }

    Ok(vec![
        "lsf".to_string(),
        format!("{remote_name}:{bucket_name}"),
        "--max-depth".to_string(),
        "1".to_string(),
    ])
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

    for component in Path::new(manifest_path).components() {
        match component {
            std::path::Component::Normal(part) => destination.push(part),
            _ => {
                return Err(format!(
                    "Manifest path must stay inside the download directory: {manifest_path}"
                ));
            }
        }
    }

    Ok(destination)
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

fn download_item_result(path: &str, status: &str, message: String) -> DownloadItemResult {
    DownloadItemResult {
        path: path.to_string(),
        status: status.to_string(),
        message,
    }
}

fn download_manifest_record_with_prefix_args(
    index_root: &Path,
    request: &DownloadRequest,
    record: &ManifestRecord,
    prefix_args: &[&str],
) -> DownloadItemResult {
    match try_download_manifest_record_with_prefix_args(index_root, request, record, prefix_args) {
        Ok(result) => result,
        Err(error) => download_item_result(&record.path, "failed", error),
    }
}

fn try_download_manifest_record_with_prefix_args(
    index_root: &Path,
    request: &DownloadRequest,
    record: &ManifestRecord,
    prefix_args: &[&str],
) -> Result<DownloadItemResult, String> {
    let destination = build_destination_path(index_root, &request.download_root, &record.path)?;
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create {}: {}", parent.display(), error))?;
    }

    if record.size == 0 {
        fs::write(&destination, [])
            .map_err(|error| format!("Failed to write {}: {}", destination.display(), error))?;
        verify_downloaded_file(&destination, record.size, &record.sha256)?;
        return Ok(download_item_result(
            &record.path,
            "createdEmpty",
            format!("created empty file {}", record.path),
        ));
    }

    let args = build_rclone_cat_args(&request.remote, &request.bucket, record)?;
    let temp_path = temp_download_path(&destination);
    if temp_path.is_file() {
        fs::remove_file(&temp_path).map_err(|error| {
            format!(
                "Failed to remove stale temp file {}: {}",
                temp_path.display(),
                error
            )
        })?;
    }

    let mut child = Command::new(&request.rclone_path)
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
    let mut temp_file = fs::File::create(&temp_path)
        .map_err(|error| format!("Failed to create {}: {}", temp_path.display(), error))?;
    if let Err(error) = std::io::copy(&mut stdout, &mut temp_file) {
        let _ = child.kill();
        let _ = child.wait();
        let stderr_text = stderr_thread
            .join()
            .unwrap_or_else(|_| "Failed to join rclone stderr reader".to_string());
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "Failed to stream rclone output to {}: {}\n{}",
            temp_path.display(),
            error,
            stderr_text
        ));
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
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "rclone cat failed for {} with status {}.\n{}",
            record.path, status, stderr_text
        ));
    }

    verify_downloaded_file(&temp_path, record.size, &record.sha256)?;
    install_verified_download(&temp_path, &destination)?;

    Ok(download_item_result(
        &record.path,
        "downloaded",
        format!("downloaded {}", record.path),
    ))
}

fn download_records(
    index_root: &Path,
    request: &DownloadRequest,
    records: Vec<ManifestRecord>,
) -> Vec<DownloadItemResult> {
    download_records_with_prefix_args(index_root, request, records, &[])
}

fn download_records_with_prefix_args(
    index_root: &Path,
    request: &DownloadRequest,
    records: Vec<ManifestRecord>,
    prefix_args: &[&str],
) -> Vec<DownloadItemResult> {
    let mut deduped = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for record in records {
        if seen.insert(record.path.clone()) {
            deduped.push(record);
        }
    }

    if request.download_jobs == 1 || deduped.len() <= 1 {
        return deduped
            .iter()
            .map(|record| {
                download_manifest_record_with_prefix_args(index_root, request, record, prefix_args)
            })
            .collect();
    }

    let queue = Arc::new(Mutex::new(VecDeque::from(deduped)));
    let results = Arc::new(Mutex::new(Vec::new()));
    let worker_count = usize::min(request.download_jobs as usize, request.paths.len());

    thread::scope(|scope| {
        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let results = Arc::clone(&results);

            scope.spawn(move || loop {
                let record = {
                    let mut queue = queue.lock().expect("queue lock should not be poisoned");
                    queue.pop_front()
                };

                let Some(record) = record else {
                    return;
                };

                let result = download_manifest_record_with_prefix_args(
                    index_root,
                    request,
                    &record,
                    prefix_args,
                );
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

fn git_update_command_args() -> Vec<&'static str> {
    vec!["pull", "--ff-only"]
}

#[tauri::command]
pub fn update_manifest_from_git(index_repo_path: String) -> Result<CommandResult, String> {
    let root = resolve_index_repo_path(&index_repo_path)?;
    let output = Command::new("git")
        .args(git_update_command_args())
        .current_dir(root)
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
pub fn check_rclone_remote(
    rclone_path: String,
    remote: String,
    bucket: String,
) -> Result<CommandResult, String> {
    if rclone_path.trim().is_empty() {
        return Err("rclone path is required".to_string());
    }

    let args = build_rclone_lsf_args(&remote, &bucket)?;
    let output = Command::new(rclone_path.trim())
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
pub fn prepare_download_root(
    index_repo_path: String,
    download_root: String,
) -> Result<String, String> {
    let root = resolve_index_repo_path(&index_repo_path)?;
    let directory = prepare_download_directory(&root, &download_root)?;
    Ok(directory.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn download_selected(request: DownloadRequest) -> Result<DownloadResult, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jsonl_manifest_records() {
        let input = r#"{"path":"资料/a.pdf","object_key":"objects/a.pdf","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":123,"storage":"r2","updated_at":"2026-06-12","visibility":"private"}
{"path":"课件/b.pptx","object_key":"objects/b.pptx","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","size":456,"storage":"r2","updated_at":"2026-06-12","visibility":"private"}
"#;

        let records = parse_manifest_jsonl(input).expect("manifest should parse");

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].path, "资料/a.pdf");
        assert_eq!(records[0].object_key, "objects/a.pdf");
        assert_eq!(records[1].size, 456);
    }

    #[test]
    fn builds_rclone_cat_args_for_a_manifest_record() {
        let record = ManifestRecord {
            path: "资料/a.pdf".to_string(),
            object_key: "objects/sha256/aa/a.pdf".to_string(),
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
                "ebookneo-r2-readonly:tyut-ebooks-collection-neo/objects/sha256/aa/a.pdf"
                    .to_string(),
            ]
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
                object_key: "objects/sha256/aa/a.pdf".to_string(),
                sha256: "a".repeat(64),
                size: 123,
                storage: "r2".to_string(),
                updated_at: "2026-06-12".to_string(),
                visibility: "private".to_string(),
            },
            ManifestRecord {
                path: "课件/b.pptx".to_string(),
                object_key: "objects/sha256/bb/b.pptx".to_string(),
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
            object_key: "objects/sha256/aa/a.pdf".to_string(),
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
    fn download_records_streams_rclone_output_and_verifies_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-direct-download-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let fake_rclone = temp_dir.join("fake-rclone.ps1");
        fs::write(
            &fake_rclone,
            "[Console]::OpenStandardOutput().Write([byte[]](97,98,99), 0, 3)\r\n",
        )
        .expect("fake rclone should be written");

        let request = DownloadRequest {
            index_repo_path: temp_dir.to_string_lossy().into_owned(),
            paths: vec!["资料/a.txt".to_string()],
            download_root: temp_dir.join("downloads").to_string_lossy().into_owned(),
            rclone_path: "powershell".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 1,
        };
        let record = ManifestRecord {
            path: "资料/a.txt".to_string(),
            object_key: "objects/sha256/ba/a.txt".to_string(),
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
            &[
                "-File",
                fake_rclone
                    .to_str()
                    .expect("fake rclone path should be utf-8"),
            ],
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
    fn download_records_reports_failures_without_stopping_the_batch() {
        let temp_dir = std::env::temp_dir().join(format!(
            "ebook-neo-structured-download-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).expect("temp dir should be created");
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

        let request = DownloadRequest {
            index_repo_path: temp_dir.to_string_lossy().into_owned(),
            paths: vec!["资料/good.txt".to_string(), "资料/bad.txt".to_string()],
            download_root: temp_dir.join("downloads").to_string_lossy().into_owned(),
            rclone_path: "powershell".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 1,
        };
        let records = vec![
            ManifestRecord {
                path: "资料/good.txt".to_string(),
                object_key: "objects/sha256/good.txt".to_string(),
                sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
                    .to_string(),
                size: 3,
                storage: "r2".to_string(),
                updated_at: "2026-06-12".to_string(),
                visibility: "private".to_string(),
            },
            ManifestRecord {
                path: "资料/bad.txt".to_string(),
                object_key: "objects/sha256/bad.txt".to_string(),
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
            &[
                "-File",
                fake_rclone
                    .to_str()
                    .expect("fake rclone path should be utf-8"),
            ],
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
        };

        validate_download_request(&request).expect("download request should validate");
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
        assert_eq!(settings.theme, "light");
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
        assert_eq!(git_update_command_args(), vec!["pull", "--ff-only"]);
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
