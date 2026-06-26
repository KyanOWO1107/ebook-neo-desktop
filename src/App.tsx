import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  ChevronRight,
  Download,
  Folder,
  FolderOpen,
  Moon,
  RefreshCw,
  Search,
  Square,
  Sun,
  UploadCloud,
  Wifi,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import "./App.css";
import {
  defaultAppSettings,
  buildRecordIndex,
  buildFolderSummaries,
  buildDownloadRequestPayload,
  buildVisibleFolderSummaries,
  clampDownloadJobs,
  clampLargeFileStreams,
  clampLargeFileThresholdMiB,
  filterDownloadQueue,
  filterRecords,
  formatBytes,
  initializeDownloadQueue,
  mergeAppSettings,
  recordsForFolderSelection,
  selectedRecordsFromIndex,
  summarizeRecords,
  themeAttribute,
  updateDownloadQueueItem,
  type AppSettings,
  type DownloadQueueFilter,
  type DownloadQueueItem,
  type ManifestRecord,
} from "./manifest";
import { VirtualList } from "./VirtualList";

type DownloadItemResult = {
  path: string | null;
  status: "downloaded" | "createdEmpty" | "failed" | "canceled";
  message: string;
};

type DownloadTask = {
  taskId: string;
};

type DownloadProgressEvent = {
  taskId: string;
  kind: "queued" | "started" | "progress" | "finished" | "failed" | "canceled" | "completed";
  path: string | null;
  bytesWritten: number;
  totalBytes: number;
  completedFiles: number;
  failedFiles: number;
  totalFiles: number;
  message: string;
};

type CommandResult = {
  stdout: string;
  stderr: string;
};

type SyncPlanItem = {
  path: string;
  status: "valid" | "missing" | "sizeMismatch" | "sha256Mismatch" | "typeMismatch";
  size: number;
  message: string;
};

type ExtraLocalFile = {
  path: string;
  size: number;
};

type SyncPlan = {
  totalFiles: number;
  totalBytes: number;
  validFiles: number;
  validBytes: number;
  missingFiles: number;
  missingBytes: number;
  outdatedFiles: number;
  outdatedBytes: number;
  extraFiles: number;
  extraBytes: number;
  downloadPaths: string[];
  items: SyncPlanItem[];
  extras: ExtraLocalFile[];
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
  const [isScanningSync, setIsScanningSync] = useState(false);
  const [downloadLog, setDownloadLog] = useState("等待选择...");
  const [downloadResults, setDownloadResults] = useState<DownloadItemResult[]>([]);
  const [downloadQueue, setDownloadQueue] = useState<DownloadQueueItem[]>([]);
  const [downloadQueueFilter, setDownloadQueueFilter] = useState<DownloadQueueFilter>("all");
  const [syncPlan, setSyncPlan] = useState<SyncPlan | null>(null);
  const [lastDownloadTargetRoot, setLastDownloadTargetRoot] = useState(defaultAppSettings.downloadRoot);
  const [activeDownloadTaskId, setActiveDownloadTaskId] = useState<string | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgressEvent | null>(null);
  const [downloadSettings, setDownloadSettings] = useState<AppSettings>(defaultAppSettings);
  const [activeView, setActiveView] = useState<"resources" | "downloads" | "sync" | "settings">("resources");
  const activeDownloadTaskIdRef = useRef<string | null>(null);
  const isAwaitingDownloadTaskRef = useRef(false);

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
      const preparedPath = await invoke<string>("open_download_root", {
        indexRepoPath: downloadSettings.indexRepoPath,
        downloadRoot: downloadSettings.downloadRoot,
      });
      setDownloadLog(`已打开下载目录：${preparedPath}`);
      setStatus("已打开下载目录");
    } catch (error) {
      setDownloadLog(error instanceof Error ? error.message : String(error));
      setStatus("打开下载目录失败");
    } finally {
      setIsOpeningDownloadRoot(false);
    }
  }

  async function scanSyncPlan() {
    if (isScanningSync) {
      return;
    }
    setIsScanningSync(true);
    setStatus("正在扫描同步目录...");
    try {
      const plan = await invoke<SyncPlan>("scan_sync_plan", {
        request: {
          indexRepoPath: downloadSettings.indexRepoPath,
          syncRoot: downloadSettings.syncRoot,
        },
      });
      setSyncPlan(plan);
      setStatus(
        `同步扫描完成：缺失 ${plan.missingFiles.toLocaleString()}，过期 ${plan.outdatedFiles.toLocaleString()}，额外 ${plan.extraFiles.toLocaleString()}`,
      );
      setDownloadLog(
        plan.downloadPaths.length > 0
          ? `待同步 ${plan.downloadPaths.length.toLocaleString()} 个文件到 ${downloadSettings.syncRoot}`
          : "同步目录已经与资源表一致",
      );
    } catch (error) {
      setStatus("同步扫描失败");
      setDownloadLog(error instanceof Error ? error.message : String(error));
    } finally {
      setIsScanningSync(false);
    }
  }

  useEffect(() => {
    loadSettings();
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    listen<DownloadProgressEvent>("download-progress", (event) => {
      if (disposed) {
        return;
      }
      handleDownloadProgress(event.payload);
    }).then((removeListener) => {
      if (disposed) {
        removeListener();
      } else {
        unlisten = removeListener;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const folders = useMemo(() => buildFolderSummaries(records), [records]);
  const visibleFolders = useMemo(() => buildVisibleFolderSummaries(folders, expandedRoots), [expandedRoots, folders]);
  const summary = useMemo(() => summarizeRecords(records), [records]);
  const recordIndex = useMemo(() => buildRecordIndex(records), [records]);
  const visibleRecords = useMemo(() => {
    const folderRecords = activeFolder
      ? records.filter((record) => record.path === activeFolder || record.path.startsWith(`${activeFolder}/`))
      : records;
    return filterRecords(folderRecords, query);
  }, [activeFolder, query, records]);
  const selectedRecords = useMemo(
    () => selectedRecordsFromIndex(recordIndex, selectedPaths),
    [recordIndex, selectedPaths],
  );
  const selectedBytes = selectedRecords.reduce((total, record) => total + record.size, 0);
  const filteredDownloadQueue = useMemo(
    () => filterDownloadQueue(downloadQueue, downloadQueueFilter),
    [downloadQueue, downloadQueueFilter],
  );

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

  function selectActiveFolder() {
    const folderRecords = recordsForFolderSelection(records, activeFolder, query);
    setSelectedPaths((current) => {
      const next = new Set(current);
      for (const record of folderRecords) {
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

  function handleDownloadProgress(event: DownloadProgressEvent) {
    if (!activeDownloadTaskIdRef.current && isAwaitingDownloadTaskRef.current) {
      activeDownloadTaskIdRef.current = event.taskId;
      setActiveDownloadTaskId(event.taskId);
      setDownloadProgress((current) => (current ? { ...current, taskId: event.taskId } : current));
    }

    if (activeDownloadTaskIdRef.current !== event.taskId) {
      return;
    }

    setDownloadProgress(event);
    setDownloadLog(event.message);
    if (event.path) {
      const status = downloadStatusFromProgress(event);
      setDownloadQueue((items) =>
        updateDownloadQueueItem(items, event.path as string, {
          status,
          message: event.message,
          bytesWritten: event.bytesWritten,
          totalBytes: event.totalBytes,
        }),
      );
    }
    if (event.path && ["finished", "failed", "canceled"].includes(event.kind)) {
      const status = downloadStatusFromProgress(event);
      setDownloadResults((items) => [
        ...items.filter((item) => item.path !== event.path),
        {
          path: event.path,
          status: status as DownloadItemResult["status"],
          message: event.message,
        },
      ]);
    }

    if (event.kind === "completed") {
      setIsDownloading(false);
      isAwaitingDownloadTaskRef.current = false;
      activeDownloadTaskIdRef.current = null;
      setActiveDownloadTaskId(null);
      setStatus(
        event.failedFiles > 0
          ? `下载完成，${event.failedFiles.toLocaleString()} 个文件失败`
          : `下载完成：${event.completedFiles.toLocaleString()} 个文件`,
      );
    }
  }

  function downloadStatusFromProgress(event: DownloadProgressEvent): DownloadQueueItem["status"] {
    if (event.kind === "started" || event.kind === "progress") {
      return "downloading";
    }
    if (event.kind === "finished") {
      return event.totalBytes === 0 ? "createdEmpty" : "downloaded";
    }
    if (event.kind === "failed" || event.kind === "canceled") {
      return event.kind;
    }
    return "queued";
  }

  function buildCommandPreview(paths: string[], targetRoot: string) {
    const selected = paths.flatMap((path) => {
      const record = recordIndex.get(path);
      return record ? [record] : [];
    });
    if (selected.length === 1) {
      return `${downloadSettings.rclonePath} ${
        selected[0].size >= downloadSettings.largeFileThresholdMiB * 1024 * 1024 ? "copyto" : "cat"
      } ${downloadSettings.remote}:${downloadSettings.bucket}/${selected[0].objectKey} -> ${targetRoot}/${selected[0].path}`;
    }
    return `${downloadSettings.rclonePath} cat/copyto>=${downloadSettings.largeFileThresholdMiB}MiB ${downloadSettings.remote}:${downloadSettings.bucket}/<object_key> -> ${targetRoot}/<manifest_path> ${selected
      .slice(0, 3)
      .map((record) => `'${record.path}'`)
      .join(" ")}${selected.length > 3 ? " ..." : ""} (jobs=${downloadSettings.downloadJobs}, large-streams=${downloadSettings.largeFileStreams})`;
  }

  async function downloadPaths(paths: string[], targetRoot = downloadSettings.downloadRoot) {
    if (paths.length === 0 || isDownloading) {
      return;
    }

    setIsDownloading(true);
    setLastDownloadTargetRoot(targetRoot);
    isAwaitingDownloadTaskRef.current = true;
    activeDownloadTaskIdRef.current = null;
    setActiveDownloadTaskId(null);
    setStatus(`正在下载 ${paths.length.toLocaleString()} 个文件...`);
    setDownloadLog(buildCommandPreview(paths, targetRoot));
    setDownloadResults([]);
    setDownloadQueueFilter("all");
    setDownloadQueue(initializeDownloadQueue(paths, recordIndex));
    setDownloadProgress({
      taskId: "",
      kind: "queued",
      path: null,
      bytesWritten: 0,
      totalBytes: 0,
      completedFiles: 0,
      failedFiles: 0,
      totalFiles: paths.length,
      message: `queued ${paths.length.toLocaleString()} file(s)`,
    });
    try {
      const task = await invoke<DownloadTask>("start_download", {
        request: buildDownloadRequestPayload({ ...downloadSettings, downloadRoot: targetRoot }, paths),
      });
      if (activeDownloadTaskIdRef.current && activeDownloadTaskIdRef.current !== task.taskId) {
        throw new Error(`下载任务事件不匹配：${activeDownloadTaskIdRef.current} != ${task.taskId}`);
      }
      isAwaitingDownloadTaskRef.current = false;
      activeDownloadTaskIdRef.current = task.taskId;
      setActiveDownloadTaskId(task.taskId);
      setDownloadProgress((current) => (current ? { ...current, taskId: task.taskId } : current));
      setStatus(`下载任务已开始：${paths.length.toLocaleString()} 个文件`);
    } catch (error) {
      isAwaitingDownloadTaskRef.current = false;
      activeDownloadTaskIdRef.current = null;
      setActiveDownloadTaskId(null);
      setDownloadLog(error instanceof Error ? error.message : String(error));
      setDownloadResults([]);
      setStatus("下载失败");
      setIsDownloading(false);
    }
  }

  async function cancelActiveDownload() {
    if (!activeDownloadTaskId) {
      return;
    }
    try {
      await invoke<CommandResult>("cancel_download", { taskId: activeDownloadTaskId });
      setStatus("正在取消下载...");
      setDownloadLog(`正在取消下载任务：${activeDownloadTaskId}`);
    } catch (error) {
      setStatus("取消下载失败");
      setDownloadLog(error instanceof Error ? error.message : String(error));
    }
  }

  async function downloadSelected() {
    await downloadPaths(selectedRecords.map((record) => record.path));
  }

  async function syncPlannedFiles() {
    if (!syncPlan || syncPlan.downloadPaths.length === 0) {
      return;
    }
    setActiveView("downloads");
    await downloadPaths(syncPlan.downloadPaths, downloadSettings.syncRoot);
  }

  async function retryFailedDownloads() {
    const failedPaths = downloadResults
      .filter((item) => item.status === "failed" && item.path)
      .map((item) => item.path as string);
    await downloadPaths(failedPaths, lastDownloadTargetRoot);
  }

  const failedDownloadCount = downloadResults.filter((item) => item.status === "failed").length;
  const canceledDownloadCount = downloadResults.filter((item) => item.status === "canceled").length;
  const currentFileName = downloadProgress?.path?.split("/").pop() ?? "等待文件...";
  const currentByteProgress =
    downloadProgress && downloadProgress.totalBytes > 0
      ? `${formatBytes(downloadProgress.bytesWritten)} / ${formatBytes(downloadProgress.totalBytes)}`
      : "等待进度...";
  const totalProgressCount =
    (downloadProgress?.completedFiles ?? 0) + (downloadProgress?.failedFiles ?? 0) + canceledDownloadCount;
  const totalProgressFiles = downloadProgress?.totalFiles ?? 0;
  const overallProgressPercent =
    totalProgressFiles > 0 ? Math.min(100, Math.round((totalProgressCount / totalProgressFiles) * 100)) : 0;

  const syncPanel = (
    <div className="table-panel sync-view">
      <div className="panel-head">
        <div>
          <h2>同步</h2>
          <p>{syncPlan ? `同步目录：${downloadSettings.syncRoot}` : "扫描同步目录后生成只读同步计划"}</p>
        </div>
        <div className="selection-actions">
          <button type="button" onClick={scanSyncPlan} disabled={isScanningSync}>
            {isScanningSync ? "扫描中" : "扫描同步"}
          </button>
          <button
            type="button"
            onClick={syncPlannedFiles}
            disabled={!syncPlan || syncPlan.downloadPaths.length === 0 || isDownloading}
          >
            开始同步
          </button>
        </div>
      </div>

      <div className="sync-summary" aria-label="同步统计">
        <span>有效 {syncPlan?.validFiles.toLocaleString() ?? 0}</span>
        <span>缺失 {syncPlan?.missingFiles.toLocaleString() ?? 0}</span>
        <span>过期 {syncPlan?.outdatedFiles.toLocaleString() ?? 0}</span>
        <span>额外 {syncPlan?.extraFiles.toLocaleString() ?? 0}</span>
      </div>

      <div className="sync-lists">
        <section aria-label="待同步文件">
          <h3>待同步文件</h3>
          {!syncPlan && <p className="empty-state">点击“扫描同步”后会列出缺失或校验不一致的文件。</p>}
          {syncPlan && syncPlan.downloadPaths.length === 0 && (
            <p className="empty-state">当前同步目录中没有需要下载的文件。</p>
          )}
          {syncPlan?.items
            .filter((item) => item.status !== "valid")
            .slice(0, 200)
            .map((item) => (
              <div className="sync-row" data-status={item.status} key={item.path}>
                <span>
                  {item.status === "missing"
                    ? "缺失"
                    : item.status === "sizeMismatch"
                      ? "大小"
                      : item.status === "sha256Mismatch"
                        ? "校验"
                        : "类型"}
                </span>
                <strong title={item.path}>{item.path}</strong>
                <small>{formatBytes(item.size)}</small>
                <small title={item.message}>{item.message}</small>
              </div>
            ))}
        </section>

        <section aria-label="额外本地文件">
          <h3>额外本地文件</h3>
          {!syncPlan && <p className="empty-state">额外文件只会展示，不会自动删除。</p>}
          {syncPlan && syncPlan.extras.length === 0 && <p className="empty-state">未发现额外本地文件。</p>}
          {syncPlan?.extras.slice(0, 200).map((extra) => (
            <div className="sync-row extra" key={extra.path}>
              <span>额外</span>
              <strong title={extra.path}>{extra.path}</strong>
              <small>{formatBytes(extra.size)}</small>
              <small>not in manifest</small>
            </div>
          ))}
        </section>
      </div>
    </div>
  );

  const settingsPanel = (
    <div className="table-panel settings-view">
      <div className="panel-head">
        <div>
          <h2>设置</h2>
          <p>{status}</p>
        </div>
        <div className="selection-actions">
          <button type="button" onClick={() => saveCurrentSettings()} disabled={isSavingSettings}>
            {isSavingSettings ? "保存中" : "保存设置"}
          </button>
          <button type="button" onClick={checkRcloneRemote} disabled={isCheckingRemote}>
            检查 R2
          </button>
          <button type="button" onClick={openDownloadRoot} disabled={isOpeningDownloadRoot}>
            打开目录
          </button>
        </div>
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
          <span>同步目录</span>
          <input
            value={downloadSettings.syncRoot}
            onChange={(event) => {
              const value = event.currentTarget.value;
              setDownloadSettings((settings) => ({ ...settings, syncRoot: value }));
            }}
          />
        </label>
        <label>
          <span>rclone</span>
          <input
            value={downloadSettings.rclonePath}
            onChange={(event) => {
              const value = event.currentTarget.value;
              setDownloadSettings((settings) => ({ ...settings, rclonePath: value }));
            }}
          />
        </label>
        <label>
          <span>Remote</span>
          <input
            value={downloadSettings.remote}
            onChange={(event) => {
              const value = event.currentTarget.value;
              setDownloadSettings((settings) => ({ ...settings, remote: value }));
            }}
          />
        </label>
        <label>
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
        <label>
          <span>大文件阈值</span>
          <input
            min={1}
            max={4096}
            type="number"
            value={downloadSettings.largeFileThresholdMiB}
            onChange={(event) => {
              const value = Number(event.currentTarget.value);
              setDownloadSettings((settings) => ({
                ...settings,
                largeFileThresholdMiB: clampLargeFileThresholdMiB(value),
              }));
            }}
          />
        </label>
        <label>
          <span>大文件线程</span>
          <input
            min={1}
            max={16}
            type="number"
            value={downloadSettings.largeFileStreams}
            onChange={(event) => {
              const value = Number(event.currentTarget.value);
              setDownloadSettings((settings) => ({
                ...settings,
                largeFileStreams: clampLargeFileStreams(value),
              }));
            }}
          />
        </label>
        <label className="settings-switch settings-wide">
          <input
            type="checkbox"
            checked={downloadSettings.showLargeFileProgress}
            onChange={(event) => {
              const checked = event.currentTarget.checked;
              setDownloadSettings((settings) => ({ ...settings, showLargeFileProgress: checked }));
            }}
          />
          <span>大文件下载进度展示</span>
        </label>
        <label className="settings-switch settings-wide">
          <input
            type="checkbox"
            checked={downloadSettings.theme === "dark"}
            onChange={(event) => {
              const checked = event.currentTarget.checked;
              setDownloadSettings((settings) => ({ ...settings, theme: checked ? "dark" : "light" }));
            }}
          />
          <span>暗色模式</span>
        </label>
      </div>

    </div>
  );

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
          <div className="view-tabs" aria-label="主视图">
            <button
              type="button"
              data-active={activeView === "resources"}
              onClick={() => setActiveView("resources")}
            >
              资料
            </button>
            <button
              type="button"
              data-active={activeView === "downloads"}
              onClick={() => setActiveView("downloads")}
            >
              下载
            </button>
            <button
              type="button"
              data-active={activeView === "sync"}
              onClick={() => setActiveView("sync")}
            >
              同步
            </button>
            <button
              type="button"
              data-active={activeView === "settings"}
              onClick={() => setActiveView("settings")}
            >
              设置
            </button>
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
          {activeView === "resources" ? (
            <div className="table-panel">
              <div className="panel-head">
                <div>
                  <h2>{activeFolder ?? "全部资料"}</h2>
                  <p className="panel-meta">
                    <span>{visibleRecords.length.toLocaleString()} 项匹配</span>
                    <span>{status}</span>
                  </p>
                </div>
                <div className="selection-actions">
                  <button type="button" onClick={selectVisible} disabled={visibleRecords.length === 0}>
                    选中当前列表
                  </button>
                  <button type="button" onClick={selectActiveFolder} disabled={!activeFolder}>
                    选中当前目录
                  </button>
                  <button type="button" onClick={clearSelection} disabled={selectedPaths.size === 0}>
                    清空
                  </button>
                </div>
              </div>

              <div className="resource-table" role="table" aria-label="资料列表区域">
                <div className="resource-row header" role="row">
                  <span></span>
                  <span>路径</span>
                  <span>大小</span>
                  <span>更新</span>
                </div>
                {visibleRecords.length === 0 ? (
                  <p className="empty-state">没有匹配的资料。</p>
                ) : (
                  <VirtualList
                    ariaLabel="资料列表"
                    className="resource-virtual-list"
                    height={480}
                    itemCount={visibleRecords.length}
                    rowHeight={40}
                    renderRow={(index) => {
                      const record = visibleRecords[index];
                      return (
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
                      );
                    }}
                  />
                )}
              </div>
            </div>
          ) : activeView === "downloads" ? (
            <div className="table-panel downloads-view">
              <div className="panel-head">
                <div>
                  <h2>下载</h2>
                  <p>
                    {downloadQueue.length > 0
                      ? `${totalProgressCount.toLocaleString()} / ${totalProgressFiles.toLocaleString()}`
                      : "暂无下载任务"}
                  </p>
                </div>
                <div className="selection-actions">
                  <select
                    aria-label="下载筛选"
                    value={downloadQueueFilter}
                    onChange={(event) => setDownloadQueueFilter(event.currentTarget.value as DownloadQueueFilter)}
                  >
                    <option value="all">全部</option>
                    <option value="active">进行中</option>
                    <option value="failed">失败</option>
                    <option value="completed">完成</option>
                  </select>
                  <button type="button" onClick={retryFailedDownloads} disabled={failedDownloadCount === 0 || isDownloading}>
                    重试失败
                  </button>
                  <button type="button" onClick={openDownloadRoot} disabled={isOpeningDownloadRoot}>
                    打开目录
                  </button>
                </div>
              </div>

              <div className="download-task-list" aria-label="下载任务列表区域">
                {downloadQueue.length === 0 && <p className="empty-state">开始下载后会在这里显示完整任务队列。</p>}
                {downloadQueue.length > 0 && filteredDownloadQueue.length === 0 && (
                  <p className="empty-state">当前筛选条件下没有下载项。</p>
                )}
                {filteredDownloadQueue.length > 0 && (
                  <VirtualList
                    ariaLabel="下载任务列表"
                    className="download-virtual-list"
                    height={500}
                    itemCount={filteredDownloadQueue.length}
                    rowHeight={56}
                    renderRow={(index) => {
                      const item = filteredDownloadQueue[index];
                      return (
                        <div className="download-task-row" data-status={item.status} key={item.path}>
                          <span className="task-status">
                            {item.status === "failed"
                              ? "失败"
                              : item.status === "createdEmpty"
                                ? "空文件"
                                : item.status === "canceled"
                                  ? "取消"
                                  : item.status === "downloaded"
                                    ? "完成"
                                    : item.status === "downloading"
                                      ? "下载中"
                                      : "排队"}
                          </span>
                          <strong title={item.path}>{item.path}</strong>
                          <small>
                            {item.totalBytes > 0
                              ? `${formatBytes(item.bytesWritten)} / ${formatBytes(item.totalBytes)}`
                              : "等待进度"}
                          </small>
                          <small title={item.message}>{item.message}</small>
                        </div>
                      );
                    }}
                  />
                )}
              </div>
            </div>
          ) : activeView === "sync" ? (
            syncPanel
          ) : (
            settingsPanel
          )}

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
            {downloadProgress && (
              <div className="progress-panel" aria-label="下载进度">
                <div className="progress-meta">
                  <span>{downloadProgress.kind === "completed" ? "任务完成" : "实时进度"}</span>
                  <strong>
                    {totalProgressCount.toLocaleString()} / {totalProgressFiles.toLocaleString()}
                  </strong>
                </div>
                <div className="progress-track" aria-label="整体下载进度">
                  <span style={{ width: `${overallProgressPercent}%` }} />
                </div>
                <div className="progress-current">
                  <strong title={downloadProgress.path ?? undefined}>{currentFileName}</strong>
                  <small>{currentByteProgress}</small>
                </div>
                <small className="progress-counts">
                  完成 {downloadProgress.completedFiles.toLocaleString()} · 失败{" "}
                  {downloadProgress.failedFiles.toLocaleString()} · 取消 {canceledDownloadCount.toLocaleString()}
                </small>
              </div>
            )}
            {activeDownloadTaskId && (
              <button
                className="secondary-action danger-action"
                type="button"
                onClick={cancelActiveDownload}
              >
                <Square size={16} />
                取消下载
              </button>
            )}
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
