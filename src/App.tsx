import { invoke } from "@tauri-apps/api/core";
import { openPath } from "@tauri-apps/plugin-opener";
import {
  ChevronRight,
  Download,
  Folder,
  FolderOpen,
  Moon,
  RefreshCw,
  Save,
  Search,
  Sun,
  UploadCloud,
  Wifi,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import "./App.css";
import {
  defaultAppSettings,
  buildFolderSummaries,
  buildVisibleFolderSummaries,
  clampDownloadJobs,
  filterRecords,
  formatBytes,
  mergeAppSettings,
  summarizeRecords,
  themeAttribute,
  type AppSettings,
  type ManifestRecord,
} from "./manifest";

type DownloadResult = {
  stdout: string;
  stderr: string;
};

type CommandResult = {
  stdout: string;
  stderr: string;
};

function App() {
  const [records, setRecords] = useState<ManifestRecord[]>([]);
  const [query, setQuery] = useState("");
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(new Set());
  const [activeFolder, setActiveFolder] = useState<string | null>(null);
  const [expandedRoots, setExpandedRoots] = useState<Set<string>>(new Set());
  const [status, setStatus] = useState("正在加载 manifest...");
  const [isLoading, setIsLoading] = useState(true);
  const [isDownloading, setIsDownloading] = useState(false);
  const [isUpdatingManifest, setIsUpdatingManifest] = useState(false);
  const [isSavingSettings, setIsSavingSettings] = useState(false);
  const [isCheckingRemote, setIsCheckingRemote] = useState(false);
  const [isOpeningDownloadRoot, setIsOpeningDownloadRoot] = useState(false);
  const [downloadLog, setDownloadLog] = useState("等待选择...");
  const [downloadSettings, setDownloadSettings] = useState<AppSettings>(defaultAppSettings);

  async function loadManifest(indexRepoPath = downloadSettings.indexRepoPath) {
    setIsLoading(true);
    try {
      const loaded = await invoke<ManifestRecord[]>("load_manifest", { indexRepoPath });
      setRecords(loaded);
      setSelectedPaths(new Set());
      setStatus(`已加载 ${loaded.length.toLocaleString()} 条记录`);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoading(false);
    }
  }

  async function loadSettings() {
    try {
      const settings = await invoke<Partial<AppSettings>>("load_settings");
      const merged = mergeAppSettings(settings);
      setDownloadSettings(merged);
      await loadManifest(merged.indexRepoPath);
    } catch (error) {
      setStatus(`设置加载失败：${error instanceof Error ? error.message : String(error)}`);
    }
  }

  async function saveCurrentSettings(settings = downloadSettings) {
    setIsSavingSettings(true);
    try {
      const saved = await invoke<AppSettings>("save_settings", { settings });
      setDownloadSettings(mergeAppSettings(saved));
      setStatus("设置已保存");
    } catch (error) {
      setStatus(`设置保存失败：${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setIsSavingSettings(false);
    }
  }

  async function toggleTheme() {
    const nextSettings: AppSettings = {
      ...downloadSettings,
      theme: downloadSettings.theme === "dark" ? "light" : "dark",
    };
    setDownloadSettings(nextSettings);
    await saveCurrentSettings(nextSettings);
  }

  async function updateManifestFromGit() {
    if (isUpdatingManifest) {
      return;
    }
    setIsUpdatingManifest(true);
    setStatus("正在从 GitHub 更新资源表...");
    try {
      const result = await invoke<CommandResult>("update_manifest_from_git", {
        indexRepoPath: downloadSettings.indexRepoPath,
      });
      const output = [result.stdout.trim(), result.stderr.trim()].filter(Boolean).join("\n\n");
      setDownloadLog(output || "资源表已更新。");
      await loadManifest(downloadSettings.indexRepoPath);
      setStatus("资源表已更新并重新加载");
    } catch (error) {
      setDownloadLog(error instanceof Error ? error.message : String(error));
      setStatus("资源表更新失败");
    } finally {
      setIsUpdatingManifest(false);
    }
  }

  async function checkRcloneRemote() {
    if (isCheckingRemote) {
      return;
    }
    setIsCheckingRemote(true);
    setStatus("正在检查 R2 只读连接...");
    try {
      const result = await invoke<CommandResult>("check_rclone_remote", {
        rclonePath: downloadSettings.rclonePath,
        remote: downloadSettings.remote,
        bucket: downloadSettings.bucket,
      });
      const output = [result.stdout.trim(), result.stderr.trim()].filter(Boolean).join("\n\n");
      setDownloadLog(output || "R2 连接正常");
      setStatus("R2 连接正常");
    } catch (error) {
      setDownloadLog(error instanceof Error ? error.message : String(error));
      setStatus("R2 连接检查失败");
    } finally {
      setIsCheckingRemote(false);
    }
  }

  async function openDownloadRoot() {
    if (isOpeningDownloadRoot) {
      return;
    }
    setIsOpeningDownloadRoot(true);
    setStatus("正在打开下载目录...");
    try {
      const preparedPath = await invoke<string>("prepare_download_root", {
        indexRepoPath: downloadSettings.indexRepoPath,
        downloadRoot: downloadSettings.downloadRoot,
      });
      await openPath(preparedPath);
      setDownloadLog(`已打开下载目录：${preparedPath}`);
      setStatus("已打开下载目录");
    } catch (error) {
      setDownloadLog(error instanceof Error ? error.message : String(error));
      setStatus("打开下载目录失败");
    } finally {
      setIsOpeningDownloadRoot(false);
    }
  }

  useEffect(() => {
    loadSettings();
  }, []);

  const folders = useMemo(() => buildFolderSummaries(records), [records]);
  const visibleFolders = useMemo(() => buildVisibleFolderSummaries(folders, expandedRoots), [expandedRoots, folders]);
  const summary = useMemo(() => summarizeRecords(records), [records]);
  const visibleRecords = useMemo(() => {
    const folderRecords = activeFolder
      ? records.filter((record) => record.path === activeFolder || record.path.startsWith(`${activeFolder}/`))
      : records;
    return filterRecords(folderRecords, query).slice(0, 500);
  }, [activeFolder, query, records]);
  const selectedRecords = useMemo(
    () => records.filter((record) => selectedPaths.has(record.path)),
    [records, selectedPaths],
  );
  const selectedBytes = selectedRecords.reduce((total, record) => total + record.size, 0);

  function togglePath(path: string) {
    setSelectedPaths((current) => {
      const next = new Set(current);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }

  function selectVisible() {
    setSelectedPaths((current) => {
      const next = new Set(current);
      for (const record of visibleRecords) {
        next.add(record.path);
      }
      return next;
    });
  }

  function clearSelection() {
    setSelectedPaths(new Set());
  }

  function toggleRoot(path: string) {
    setExpandedRoots((current) => {
      const next = new Set(current);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }

  async function downloadSelected() {
    if (selectedRecords.length === 0 || isDownloading) {
      return;
    }

    setIsDownloading(true);
    setStatus(`正在下载 ${selectedRecords.length.toLocaleString()} 个文件...`);
    setDownloadLog(commandPreview);
    try {
      const result = await invoke<DownloadResult>("download_selected", {
        request: {
          indexRepoPath: downloadSettings.indexRepoPath,
          paths: selectedRecords.map((record) => record.path),
          downloadRoot: downloadSettings.downloadRoot,
          rclonePath: downloadSettings.rclonePath,
          remote: downloadSettings.remote,
          bucket: downloadSettings.bucket,
          downloadJobs: downloadSettings.downloadJobs,
        },
      });
      const output = [result.stdout.trim(), result.stderr.trim()].filter(Boolean).join("\n\n");
      setDownloadLog(output || "下载命令执行完成。");
      setStatus(`下载完成：${selectedRecords.length.toLocaleString()} 个文件`);
    } catch (error) {
      setDownloadLog(error instanceof Error ? error.message : String(error));
      setStatus("下载失败");
    } finally {
      setIsDownloading(false);
    }
  }

  const commandPreview =
    selectedRecords.length === 1
      ? `${downloadSettings.rclonePath} cat ${downloadSettings.remote}:${downloadSettings.bucket}/${selectedRecords[0].objectKey} -> ${downloadSettings.downloadRoot}/${selectedRecords[0].path}`
      : `${downloadSettings.rclonePath} cat ${downloadSettings.remote}:${downloadSettings.bucket}/<object_key> -> ${downloadSettings.downloadRoot}/<manifest_path> ${selectedRecords
          .slice(0, 3)
          .map((record) => `'${record.path}'`)
          .join(" ")}${selectedRecords.length > 3 ? " ..." : ""} (jobs=${downloadSettings.downloadJobs})`;

  return (
    <main className="app-shell" data-theme={themeAttribute(downloadSettings.theme)}>
      <aside className="sidebar" aria-label="资料分类">
        <div className="brand">
          <div className="brand-mark" aria-hidden="true">
            <img src="/favicon.png" alt="" />
          </div>
          <div>
            <h1>Ebook Neo</h1>
            <p>TYUT R2 资料索引</p>
          </div>
        </div>

        <button className="folder-button all" onClick={() => setActiveFolder(null)} data-active={activeFolder === null}>
          <Folder size={18} />
          <span>全部资料</span>
          <strong>{summary.files.toLocaleString()}</strong>
        </button>

        <div className="folder-list">
          {visibleFolders.map((folder) => (
            <div
              className="folder-row"
              key={folder.path}
              data-active={activeFolder === folder.path}
              data-depth={folder.depth}
              data-expanded={folder.depth === 0 ? expandedRoots.has(folder.path) : undefined}
            >
              {folder.depth === 0 ? (
                <button
                  className="folder-disclosure"
                  type="button"
                  onClick={() => toggleRoot(folder.path)}
                  title={expandedRoots.has(folder.path) ? "折叠目录" : "展开目录"}
                  aria-label={`${expandedRoots.has(folder.path) ? "折叠" : "展开"}${folder.label}`}
                >
                  <ChevronRight
                    size={15}
                  />
                </button>
              ) : (
                <span className="folder-disclosure" aria-hidden="true">
                  <Folder size={16} />
                </span>
              )}
              <button
                className="folder-button"
                type="button"
                onClick={() => setActiveFolder(folder.path)}
                title={folder.path}
              >
                <span>{folder.label}</span>
                <strong>{folder.files.toLocaleString()}</strong>
              </button>
            </div>
          ))}
        </div>
      </aside>

      <section className="workspace">
        <header className="toolbar">
          <div className="search-box">
            <Search size={18} />
            <input
              aria-label="搜索资料"
              value={query}
              onChange={(event) => setQuery(event.currentTarget.value)}
              placeholder="搜索路径、文件名或扩展名"
            />
          </div>
          <button
            className="text-button"
            type="button"
            onClick={updateManifestFromGit}
            disabled={isUpdatingManifest || isLoading}
            title="从 GitHub 拉取最新资源表"
          >
            <UploadCloud size={18} />
            <span>{isUpdatingManifest ? "更新中" : "更新资源表"}</span>
          </button>
          <button
            className="icon-button"
            type="button"
            onClick={toggleTheme}
            disabled={isSavingSettings}
            title={downloadSettings.theme === "dark" ? "切换到亮色" : "切换到暗色"}
          >
            {downloadSettings.theme === "dark" ? <Sun size={18} /> : <Moon size={18} />}
          </button>
          <button className="icon-button" type="button" onClick={() => loadManifest()} disabled={isLoading} title="重新加载">
            <RefreshCw size={18} />
          </button>
        </header>

        <section className="stats-grid" aria-label="资料统计">
          <div className="stat">
            <span>文件</span>
            <strong>{summary.files.toLocaleString()}</strong>
          </div>
          <div className="stat">
            <span>容量</span>
            <strong>{formatBytes(summary.bytes)}</strong>
          </div>
          <div className="stat">
            <span>分类</span>
            <strong>{summary.roots}</strong>
          </div>
          <div className="stat">
            <span>选中</span>
            <strong>{selectedRecords.length.toLocaleString()}</strong>
          </div>
        </section>

        <section className="content-grid">
          <div className="table-panel">
            <div className="panel-head">
              <div>
                <h2>{activeFolder ?? "全部资料"}</h2>
                <p>{status}</p>
              </div>
              <div className="selection-actions">
                <button type="button" onClick={selectVisible} disabled={visibleRecords.length === 0}>
                  选中当前列表
                </button>
                <button type="button" onClick={clearSelection} disabled={selectedPaths.size === 0}>
                  清空
                </button>
              </div>
            </div>

            <div className="resource-table" role="table" aria-label="资料列表">
              <div className="resource-row header" role="row">
                <span></span>
                <span>路径</span>
                <span>大小</span>
                <span>更新</span>
              </div>
              {visibleRecords.map((record) => (
                <label className="resource-row" key={record.path} role="row">
                  <input
                    type="checkbox"
                    checked={selectedPaths.has(record.path)}
                    onChange={() => togglePath(record.path)}
                  />
                  <span className="path-cell" title={record.path}>
                    {record.path}
                  </span>
                  <span>{formatBytes(record.size)}</span>
                  <span>{record.updatedAt}</span>
                </label>
              ))}
            </div>
          </div>

          <aside className="download-panel" aria-label="下载队列">
            <div className="panel-head compact">
              <h2>下载队列</h2>
              <p>{formatBytes(selectedBytes)}</p>
            </div>
            <div className="queue-list">
              {selectedRecords.slice(0, 8).map((record) => (
                <div className="queue-item" key={record.path}>
                  <span>{record.path.split("/").pop()}</span>
                  <small>{formatBytes(record.size)}</small>
                </div>
              ))}
              {selectedRecords.length === 0 && <p className="empty-state">从左侧列表选择文件后开始下载。</p>}
              {selectedRecords.length > 8 && <p className="empty-state">另有 {selectedRecords.length - 8} 个文件已选中。</p>}
            </div>
            <div className="settings-grid" aria-label="下载设置">
              <label className="settings-wide">
                <span>索引仓库</span>
                <input
                  value={downloadSettings.indexRepoPath}
                  onChange={(event) => {
                    const value = event.currentTarget.value;
                    setDownloadSettings((settings) => ({ ...settings, indexRepoPath: value }));
                  }}
                />
              </label>
              <label className="settings-wide">
                <span>下载目录</span>
                <input
                  value={downloadSettings.downloadRoot}
                  onChange={(event) => {
                    const value = event.currentTarget.value;
                    setDownloadSettings((settings) => ({ ...settings, downloadRoot: value }));
                  }}
                />
              </label>
              <label className="settings-wide">
                <span>rclone</span>
                <input
                  value={downloadSettings.rclonePath}
                  onChange={(event) => {
                    const value = event.currentTarget.value;
                    setDownloadSettings((settings) => ({ ...settings, rclonePath: value }));
                  }}
                />
              </label>
              <label className="settings-wide">
                <span>Remote</span>
                <input
                  value={downloadSettings.remote}
                  onChange={(event) => {
                    const value = event.currentTarget.value;
                    setDownloadSettings((settings) => ({ ...settings, remote: value }));
                  }}
                />
              </label>
              <label className="settings-wide">
                <span>Bucket</span>
                <input
                  value={downloadSettings.bucket}
                  onChange={(event) => {
                    const value = event.currentTarget.value;
                    setDownloadSettings((settings) => ({ ...settings, bucket: value }));
                  }}
                />
              </label>
              <label>
                <span>并发</span>
                <input
                  min={1}
                  max={16}
                  type="number"
                  value={downloadSettings.downloadJobs}
                  onChange={(event) => {
                    const value = Number(event.currentTarget.value);
                    setDownloadSettings((settings) => ({
                      ...settings,
                      downloadJobs: clampDownloadJobs(value),
                    }));
                  }}
                />
              </label>
            </div>
            <button
              className="secondary-action"
              type="button"
              onClick={() => saveCurrentSettings()}
              disabled={isSavingSettings}
            >
              <Save size={16} />
              {isSavingSettings ? "保存中" : "保存设置"}
            </button>
            <div className="utility-actions">
              <button
                className="secondary-action"
                type="button"
                onClick={checkRcloneRemote}
                disabled={isCheckingRemote}
              >
                <Wifi size={16} />
                {isCheckingRemote ? "检查中" : "检查 R2"}
              </button>
              <button
                className="secondary-action"
                type="button"
                onClick={openDownloadRoot}
                disabled={isOpeningDownloadRoot}
              >
                <FolderOpen size={16} />
                {isOpeningDownloadRoot ? "打开中" : "打开目录"}
              </button>
            </div>
            <pre className="command-preview">{selectedRecords.length > 0 ? downloadLog : "等待选择..."}</pre>
            <button
              className="primary-action"
              type="button"
              disabled={selectedRecords.length === 0 || isDownloading}
              onClick={downloadSelected}
            >
              <Download size={18} />
              {isDownloading ? "下载中" : "开始下载"}
            </button>
          </aside>
        </section>
      </section>
    </main>
  );
}

export default App;
