export type ManifestRecord = {
  path: string;
  objectKey: string;
  sha256: string;
  size: number;
  storage: string;
  updatedAt: string;
  visibility: string;
};

export type CollectionSummary = {
  files: number;
  bytes: number;
  roots: number;
};

export type FolderSummary = {
  path: string;
  label: string;
  files: number;
  bytes: number;
  depth: number;
};

export type ThemeMode = "light" | "dark";

export type AppSettings = {
  indexRepoPath: string;
  downloadRoot: string;
  rclonePath: string;
  remote: string;
  bucket: string;
  downloadJobs: number;
  largeFileThresholdMiB: number;
  largeFileStreams: number;
  showLargeFileProgress: boolean;
  theme: ThemeMode;
};

export type DownloadRequestPayload = {
  indexRepoPath: string;
  paths: string[];
  downloadRoot: string;
  rclonePath: string;
  remote: string;
  bucket: string;
  downloadJobs: number;
  largeFileThresholdMiB: number;
  largeFileStreams: number;
  showLargeFileProgress: boolean;
};

export const defaultAppSettings: AppSettings = {
  indexRepoPath: "../TYUT-ebooks-collection-neo",
  downloadRoot: "downloads/gui",
  rclonePath: "rclone",
  remote: "ebookneo-r2-readonly",
  bucket: "tyut-ebooks-collection-neo",
  downloadJobs: 4,
  largeFileThresholdMiB: 20,
  largeFileStreams: 8,
  showLargeFileProgress: true,
  theme: "light",
};

export function buildDownloadRequestPayload(settings: AppSettings, paths: string[]): DownloadRequestPayload {
  return {
    indexRepoPath: settings.indexRepoPath,
    paths,
    downloadRoot: settings.downloadRoot,
    rclonePath: settings.rclonePath,
    remote: settings.remote,
    bucket: settings.bucket,
    downloadJobs: settings.downloadJobs,
    largeFileThresholdMiB: settings.largeFileThresholdMiB,
    largeFileStreams: settings.largeFileStreams,
    showLargeFileProgress: settings.showLargeFileProgress,
  };
}

export function themeAttribute(theme: string): ThemeMode {
  return theme === "dark" ? "dark" : "light";
}

export function clampDownloadJobs(jobs: number): number {
  if (!Number.isFinite(jobs)) {
    return defaultAppSettings.downloadJobs;
  }
  return Math.min(16, Math.max(1, Math.trunc(jobs)));
}

export function clampLargeFileThresholdMiB(value: number): number {
  if (!Number.isFinite(value)) {
    return defaultAppSettings.largeFileThresholdMiB;
  }
  return Math.min(4096, Math.max(1, Math.trunc(value)));
}

export function clampLargeFileStreams(value: number): number {
  if (!Number.isFinite(value)) {
    return defaultAppSettings.largeFileStreams;
  }
  return Math.min(16, Math.max(1, Math.trunc(value)));
}

export function mergeAppSettings(settings: Partial<AppSettings>): AppSettings {
  return {
    ...defaultAppSettings,
    ...settings,
    downloadJobs: clampDownloadJobs(settings.downloadJobs ?? defaultAppSettings.downloadJobs),
    largeFileThresholdMiB: clampLargeFileThresholdMiB(
      settings.largeFileThresholdMiB ?? defaultAppSettings.largeFileThresholdMiB,
    ),
    largeFileStreams: clampLargeFileStreams(settings.largeFileStreams ?? defaultAppSettings.largeFileStreams),
    showLargeFileProgress:
      typeof settings.showLargeFileProgress === "boolean"
        ? settings.showLargeFileProgress
        : defaultAppSettings.showLargeFileProgress,
    theme: themeAttribute(settings.theme ?? defaultAppSettings.theme),
  };
}

export function summarizeRecords(records: ManifestRecord[]): CollectionSummary {
  const roots = new Set(records.map((record) => record.path.split("/")[0]).filter(Boolean));
  return {
    files: records.length,
    bytes: records.reduce((total, record) => total + record.size, 0),
    roots: roots.size,
  };
}

export function buildFolderSummaries(records: ManifestRecord[]): FolderSummary[] {
  const byPath = new Map<string, FolderSummary>();

  for (const record of records) {
    const segments = record.path.split("/").filter(Boolean);
    const folderPaths = segments.length >= 2 ? [segments[0], `${segments[0]}/${segments[1]}`] : [record.path];

    for (const [index, path] of folderPaths.entries()) {
      const pathSegments = path.split("/");
      const label = pathSegments[pathSegments.length - 1] || path;
      const summary = byPath.get(path) ?? { path, label, files: 0, bytes: 0, depth: index };
      summary.files += 1;
      summary.bytes += record.size;
      byPath.set(path, summary);
    }
  }

  return Array.from(byPath.values());
}

export function buildVisibleFolderSummaries(
  folders: FolderSummary[],
  expandedRoots: ReadonlySet<string>,
): FolderSummary[] {
  return folders.filter((folder) => {
    if (folder.depth === 0) {
      return true;
    }

    const root = folder.path.split("/")[0];
    return expandedRoots.has(root);
  });
}

export function filterRecords(records: ManifestRecord[], query: string): ManifestRecord[] {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  if (!normalizedQuery) {
    return records;
  }

  return records.filter((record) => record.path.toLocaleLowerCase().includes(normalizedQuery));
}

export function recordsForFolderSelection(
  records: ManifestRecord[],
  activeFolder: string | null,
  query: string,
): ManifestRecord[] {
  const folderRecords = activeFolder
    ? records.filter((record) => record.path === activeFolder || record.path.startsWith(`${activeFolder}/`))
    : records;
  return filterRecords(folderRecords, query);
}

export function formatBytes(bytes: number): string {
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  if (unitIndex === 0) {
    return `${value} B`;
  }

  return `${value.toFixed(3)} ${units[unitIndex]}`;
}
