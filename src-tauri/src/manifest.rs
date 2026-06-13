use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
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

#[derive(Debug, PartialEq, Serialize)]
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
            let raw: RawManifestRecord = serde_json::from_str(line)
                .map_err(|error| format!("Invalid manifest JSON on line {}: {}", index + 1, error))?;
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
    let contents = fs::read_to_string(path).map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
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
    pub python_command: String,
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
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_index_repo_path")]
    pub index_repo_path: String,
    pub download_root: String,
    pub python_command: String,
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
            python_command: "python".to_string(),
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
    if settings.python_command.trim().is_empty() {
        return Err("Python command is required".to_string());
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
    let contents =
        fs::read_to_string(path).map_err(|error| format!("Failed to read {}: {}", path.display(), error))?;
    let settings: AppSettings = serde_json::from_str(&contents)
        .map_err(|error| format!("Invalid settings JSON in {}: {}", path.display(), error))?;
    validate_settings(&settings)?;
    Ok(settings)
}

fn save_settings_to_path(path: &Path, settings: &AppSettings) -> Result<(), String> {
    validate_settings(settings)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Failed to create {}: {}", parent.display(), error))?;
    }
    let contents = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(path, contents).map_err(|error| format!("Failed to write {}: {}", path.display(), error))
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
    let trimmed = index_repo_path.trim();
    if trimmed.is_empty() {
        return Err("Index repository path is required".to_string());
    }

    let configured_path = PathBuf::from(trimmed);
    let candidate = if configured_path.is_absolute() {
        configured_path
    } else {
        std::env::current_dir()
            .map_err(|error| format!("Failed to resolve current directory: {}", error))?
            .join(configured_path)
    };

    let root = fs::canonicalize(&candidate)
        .map_err(|error| format!("Failed to resolve index repository path {}: {}", candidate.display(), error))?;

    if !root.join("manifests/files.jsonl").is_file() {
        return Err(format!(
            "Index repository path {} is missing manifests/files.jsonl",
            root.display()
        ));
    }

    if !root.join("tools/fetch_objects.py").is_file() {
        return Err(format!(
            "Index repository path {} is missing tools/fetch_objects.py",
            root.display()
        ));
    }

    Ok(root)
}

fn build_fetch_command_args(request: &DownloadRequest) -> Result<Vec<String>, String> {
    if request.paths.is_empty() {
        return Err("Select at least one file before downloading".to_string());
    }
    if request.python_command.trim().is_empty() {
        return Err("Python command is required".to_string());
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

    let mut args = vec![
        "tools/fetch_objects.py".to_string(),
        "--manifest".to_string(),
        "manifests/files.jsonl".to_string(),
        "--download-root".to_string(),
        request.download_root.clone(),
        "--remote".to_string(),
        request.remote.clone(),
        "--bucket".to_string(),
        request.bucket.clone(),
        "--execute".to_string(),
        "--transfer-mode".to_string(),
        "cat".to_string(),
        "--rclone".to_string(),
        request.rclone_path.clone(),
        "--jobs".to_string(),
        request.download_jobs.to_string(),
    ];

    for path in &request.paths {
        args.push("--path".to_string());
        args.push(path.clone());
    }

    Ok(args)
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
pub fn download_selected(request: DownloadRequest) -> Result<DownloadResult, String> {
    let root = resolve_index_repo_path(&request.index_repo_path)?;
    let args = build_fetch_command_args(&request)?;
    let output = Command::new(&request.python_command)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("Failed to start download command: {}", error))?;

    let result = DownloadResult {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    };

    if output.status.success() {
        Ok(result)
    } else {
        Err(format!(
            "Download command failed with status {}.\n{}\n{}",
            output.status, result.stdout, result.stderr
        ))
    }
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
    fn builds_fetch_command_args_for_selected_paths() {
        let request = DownloadRequest {
            index_repo_path: "../TYUT-ebooks-collection-neo".to_string(),
            paths: vec!["资料/a.pdf".to_string(), "课件/b.pptx".to_string()],
            download_root: "downloads/gui".to_string(),
            python_command: "python".to_string(),
            rclone_path: "rclone".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 4,
        };

        let args = build_fetch_command_args(&request).expect("args should build");

        assert_eq!(
            args,
            vec![
                "tools/fetch_objects.py",
                "--manifest",
                "manifests/files.jsonl",
                "--download-root",
                "downloads/gui",
                "--remote",
                "ebookneo-r2-readonly",
                "--bucket",
                "tyut-ebooks-collection-neo",
                "--execute",
                "--transfer-mode",
                "cat",
                "--rclone",
                "rclone",
                "--jobs",
                "4",
                "--path",
                "资料/a.pdf",
                "--path",
                "课件/b.pptx",
            ]
        );
    }

    #[test]
    fn rejects_empty_download_selection() {
        let request = DownloadRequest {
            index_repo_path: "../TYUT-ebooks-collection-neo".to_string(),
            paths: Vec::new(),
            download_root: "downloads/gui".to_string(),
            python_command: "python".to_string(),
            rclone_path: "rclone".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 4,
        };

        let error = build_fetch_command_args(&request).expect_err("empty selection should fail");

        assert_eq!(error, "Select at least one file before downloading");
    }

    #[test]
    fn rejects_download_jobs_above_limit() {
        let request = DownloadRequest {
            index_repo_path: "../TYUT-ebooks-collection-neo".to_string(),
            paths: vec!["资料/a.pdf".to_string()],
            download_root: "downloads/gui".to_string(),
            python_command: "python".to_string(),
            rclone_path: "rclone".to_string(),
            remote: "ebookneo-r2-readonly".to_string(),
            bucket: "tyut-ebooks-collection-neo".to_string(),
            download_jobs: 17,
        };

        let error = build_fetch_command_args(&request).expect_err("too many jobs should fail");

        assert_eq!(error, "Download jobs must be between 1 and 16");
    }

    #[test]
    fn default_settings_include_download_jobs_and_light_theme() {
        let settings = default_settings();

        assert_eq!(settings.index_repo_path, "../TYUT-ebooks-collection-neo");
        assert_eq!(settings.download_root, "downloads/gui");
        assert_eq!(settings.python_command, "python");
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
            python_command: "python3".to_string(),
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
        fs::create_dir_all(temp_dir.join("tools")).expect("tools dir should be created");
        fs::create_dir_all(temp_dir.join("manifests")).expect("manifests dir should be created");
        fs::write(temp_dir.join("tools/fetch_objects.py"), "").expect("fetch tool should be created");
        fs::write(temp_dir.join("manifests/files.jsonl"), "").expect("manifest should be created");

        let expected = fs::canonicalize(&temp_dir).expect("temp path should canonicalize");
        let resolved = resolve_index_repo_path(temp_dir.to_str().expect("temp path should be utf-8"))
            .expect("index repo path should resolve");

        assert_eq!(resolved, expected);
        fs::remove_dir_all(&resolved).expect("temp dir should be removed");
    }
}
