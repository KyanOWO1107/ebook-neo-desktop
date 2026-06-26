// @vitest-environment jsdom

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import App from "./App";
import { defaultAppSettings, type ManifestRecord } from "./manifest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn())),
}));

const records: ManifestRecord[] = [
  {
    path: "资料/数据结构/a.pdf",
    objectKey: "objects/sha256/aa/a.pdf",
    sha256: "a".repeat(64),
    size: 1024,
    storage: "r2",
    updatedAt: "2026-06-12",
    visibility: "private",
  },
  {
    path: "资料/数据结构/b.pdf",
    objectKey: "objects/sha256/bb/b.pdf",
    sha256: "b".repeat(64),
    size: 2048,
    storage: "r2",
    updatedAt: "2026-06-12",
    visibility: "private",
  },
];

function makeRecord(index: number): ManifestRecord {
  const suffix = index.toString().padStart(4, "0");
  return {
    path: `资料/性能测试/file-${suffix}.pdf`,
    objectKey: `objects/sha256/${suffix}/file-${suffix}.pdf`,
    sha256: suffix.padEnd(64, "a").slice(0, 64),
    size: 1024 + index,
    storage: "r2",
    updatedAt: "2026-06-26",
    visibility: "private",
  };
}

function makeSyncPlan(count: number) {
  return {
    totalFiles: count,
    totalBytes: count * 1024,
    validFiles: 0,
    validBytes: 0,
    missingFiles: count,
    missingBytes: count * 1024,
    outdatedFiles: 0,
    outdatedBytes: 0,
    extraFiles: count,
    extraBytes: count * 12,
    downloadPaths: Array.from({ length: count }, (_, index) => `资料/同步/file-${index.toString().padStart(4, "0")}.pdf`),
    items: Array.from({ length: count }, (_, index) => ({
      path: `资料/同步/file-${index.toString().padStart(4, "0")}.pdf`,
      status: "missing",
      size: 1024,
      message: "local file is missing",
    })),
    extras: Array.from({ length: count }, (_, index) => ({
      path: `extra/local-${index.toString().padStart(4, "0")}.txt`,
      size: 12,
    })),
  };
}

function mockedInvoke() {
  return invoke as Mock;
}

function mockedListen() {
  return listen as Mock;
}

function emitDownloadProgress(payload: unknown) {
  const calls = mockedListen().mock.calls;
  const downloadProgressCall = calls.find(([eventName]) => eventName === "download-progress");
  expect(downloadProgressCall).toBeTruthy();
  const handler = downloadProgressCall?.[1] as (event: { payload: unknown }) => void;
  handler({ payload });
}

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve;
  });
  return { promise, resolve };
}

describe("App", () => {
  beforeEach(() => {
    mockedInvoke().mockImplementation((command: string) => {
      if (command === "load_settings") {
        return Promise.resolve(defaultAppSettings);
      }
      if (command === "load_manifest") {
        return Promise.resolve(records);
      }
      if (command === "check_rclone_remote") {
        return Promise.resolve({ stdout: "objects/\n", stderr: "" });
      }
      if (command === "open_download_root") {
        return Promise.resolve("E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo/downloads/gui");
      }
      if (command === "scan_sync_plan") {
        return Promise.resolve({
          totalFiles: 2,
          totalBytes: 3072,
          validFiles: 1,
          validBytes: 1024,
          missingFiles: 1,
          missingBytes: 2048,
          outdatedFiles: 0,
          outdatedBytes: 0,
          extraFiles: 1,
          extraBytes: 12,
          downloadPaths: ["资料/数据结构/b.pdf"],
          items: [
            {
              path: "资料/数据结构/a.pdf",
              status: "valid",
              size: 1024,
              message: "local file is valid",
            },
            {
              path: "资料/数据结构/b.pdf",
              status: "missing",
              size: 2048,
              message: "local file is missing",
            },
          ],
          extras: [
            {
              path: "notes/local.txt",
              size: 12,
            },
          ],
        });
      }
      if (command === "start_download") {
        return Promise.resolve({ taskId: "download-1" });
      }
      if (command === "cancel_download") {
        return Promise.resolve({ stdout: "Cancel requested for download-1", stderr: "" });
      }
      return Promise.resolve({ stdout: "", stderr: "" });
    });
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("listens for live download progress events", async () => {
    render(<App />);

    await waitFor(() => expect(mockedListen()).toHaveBeenCalledWith("download-progress", expect.any(Function)));
  });

  it("keeps settings text inputs editable when values are cleared or pasted", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getAllByText("资料/数据结构/a.pdf").length).toBeGreaterThan(0));
    fireEvent.click(screen.getByRole("button", { name: "设置" }));

    const downloadRoot = screen.getByLabelText("下载目录") as HTMLInputElement;
    const syncRoot = screen.getByLabelText("同步目录") as HTMLInputElement;
    const indexRepoPath = screen.getByLabelText("索引仓库") as HTMLInputElement;
    const largeFileThreshold = screen.getByLabelText("大文件阈值") as HTMLInputElement;
    const largeFileStreams = screen.getByLabelText("大文件线程") as HTMLInputElement;

    fireEvent.change(downloadRoot, { target: { value: "" } });
    expect(downloadRoot.value).toBe("");

    fireEvent.change(downloadRoot, { target: { value: "D:/TYUT downloads" } });
    expect(downloadRoot.value).toBe("D:/TYUT downloads");

    fireEvent.change(syncRoot, { target: { value: "" } });
    expect(syncRoot.value).toBe("");

    fireEvent.change(syncRoot, { target: { value: "D:/TYUT sync" } });
    expect(syncRoot.value).toBe("D:/TYUT sync");

    fireEvent.change(indexRepoPath, { target: { value: "" } });
    expect(indexRepoPath.value).toBe("");

    fireEvent.change(indexRepoPath, { target: { value: "E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo" } });
    expect(indexRepoPath.value).toBe("E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo");

    fireEvent.change(largeFileThreshold, { target: { value: "32" } });
    expect(largeFileThreshold.value).toBe("32");

    fireEvent.change(largeFileStreams, { target: { value: "12" } });
    expect(largeFileStreams.value).toBe("12");
  });

  it("shows persistent configuration on the settings view instead of the download queue panel", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());

    const downloadPanel = screen.getByLabelText("下载队列");
    expect(downloadPanel.textContent).not.toContain("索引仓库");
    expect(downloadPanel.textContent).not.toContain("大文件阈值");

    fireEvent.click(screen.getByRole("button", { name: "设置" }));

    expect(screen.getByRole("button", { name: "设置" }).getAttribute("data-active")).toBe("true");
    expect(screen.getByLabelText("索引仓库")).toBeTruthy();
    expect(screen.getByLabelText("下载目录")).toBeTruthy();
    expect(screen.getByLabelText("同步目录")).toBeTruthy();
    expect(screen.getByLabelText("rclone")).toBeTruthy();
    expect(screen.getByLabelText("Remote")).toBeTruthy();
    expect(screen.getByLabelText("Bucket")).toBeTruthy();
    expect(screen.getByLabelText("并发")).toBeTruthy();
    expect(screen.getByLabelText("大文件阈值")).toBeTruthy();
    expect(screen.getByLabelText("大文件线程")).toBeTruthy();
    expect(screen.getByLabelText("大文件下载进度展示")).toBeTruthy();
    expect(screen.getByLabelText("暗色模式")).toBeTruthy();
    expect(screen.getByRole("button", { name: "保存设置" })).toBeTruthy();
  });

  it("syncs the theme to the document root so page background follows dark mode", async () => {
    mockedInvoke().mockImplementation((command: string) => {
      if (command === "load_settings") {
        return Promise.resolve(defaultAppSettings);
      }
      if (command === "load_manifest") {
        return Promise.resolve(records);
      }
      if (command === "save_settings") {
        return Promise.resolve({ ...defaultAppSettings, theme: "dark" });
      }
      return Promise.resolve({ stdout: "", stderr: "" });
    });

    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());
    expect(document.documentElement.getAttribute("data-theme")).toBe("light");

    fireEvent.click(screen.getByTitle("切换到暗色"));

    await waitFor(() => expect(document.documentElement.getAttribute("data-theme")).toBe("dark"));
  });

  it("scans the saved sync folder and starts syncing only missing or outdated files", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "同步" }));
    fireEvent.click(screen.getByRole("button", { name: "扫描同步" }));

    await waitFor(() =>
      expect(mockedInvoke()).toHaveBeenCalledWith("scan_sync_plan", {
        request: {
          indexRepoPath: defaultAppSettings.indexRepoPath,
          syncRoot: defaultAppSettings.syncRoot,
        },
      }),
    );
    expect(await screen.findByText("缺失 1")).toBeTruthy();
    expect(screen.getByText("额外 1")).toBeTruthy();
    expect(screen.getByText("资料/数据结构/b.pdf")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "开始同步" }));

    await waitFor(() =>
      expect(mockedInvoke()).toHaveBeenLastCalledWith("start_download", {
        request: {
          indexRepoPath: defaultAppSettings.indexRepoPath,
          paths: ["资料/数据结构/b.pdf"],
          downloadRoot: defaultAppSettings.syncRoot,
          rclonePath: defaultAppSettings.rclonePath,
          remote: defaultAppSettings.remote,
          bucket: defaultAppSettings.bucket,
          downloadJobs: defaultAppSettings.downloadJobs,
          largeFileThresholdMiB: defaultAppSettings.largeFileThresholdMiB,
          largeFileStreams: defaultAppSettings.largeFileStreams,
          showLargeFileProgress: defaultAppSettings.showLargeFileProgress,
        },
      }),
    );
  });

  it("scans and syncs only the active folder when current-folder scope is selected", async () => {
    mockedInvoke().mockImplementation((command: string) => {
      if (command === "load_settings") {
        return Promise.resolve(defaultAppSettings);
      }
      if (command === "load_manifest") {
        return Promise.resolve(records);
      }
      if (command === "scan_sync_plan") {
        return Promise.resolve({
          totalFiles: 1,
          totalBytes: 2048,
          validFiles: 0,
          validBytes: 0,
          missingFiles: 1,
          missingBytes: 2048,
          outdatedFiles: 0,
          outdatedBytes: 0,
          extraFiles: 0,
          extraBytes: 0,
          downloadPaths: ["资料/数据结构/b.pdf"],
          items: [
            {
              path: "资料/数据结构/b.pdf",
              status: "missing",
              size: 2048,
              message: "local file is missing",
            },
          ],
          extras: [],
        });
      }
      if (command === "start_download") {
        return Promise.resolve({ taskId: "download-1" });
      }
      return Promise.resolve({ stdout: "", stderr: "" });
    });

    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "展开资料" }));
    fireEvent.click(screen.getByRole("button", { name: /数据结构/ }));
    fireEvent.click(screen.getByRole("button", { name: "同步" }));
    fireEvent.change(screen.getByLabelText("同步范围"), { target: { value: "folder" } });
    fireEvent.click(screen.getByRole("button", { name: "扫描同步" }));

    await waitFor(() =>
      expect(mockedInvoke()).toHaveBeenCalledWith("scan_sync_plan", {
        request: {
          indexRepoPath: defaultAppSettings.indexRepoPath,
          syncRoot: defaultAppSettings.syncRoot,
          scopePrefix: "资料/数据结构",
        },
      }),
    );

    fireEvent.click(await screen.findByRole("button", { name: "开始同步" }));

    await waitFor(() =>
      expect(mockedInvoke()).toHaveBeenLastCalledWith("start_download", {
        request: {
          indexRepoPath: defaultAppSettings.indexRepoPath,
          paths: ["资料/数据结构/b.pdf"],
          downloadRoot: defaultAppSettings.syncRoot,
          rclonePath: defaultAppSettings.rclonePath,
          remote: defaultAppSettings.remote,
          bucket: defaultAppSettings.bucket,
          downloadJobs: defaultAppSettings.downloadJobs,
          largeFileThresholdMiB: defaultAppSettings.largeFileThresholdMiB,
          largeFileStreams: defaultAppSettings.largeFileStreams,
          showLargeFileProgress: defaultAppSettings.showLargeFileProgress,
        },
      }),
    );
  });

  it("opens the configured sync folder from the sync view", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "同步" }));
    fireEvent.click(screen.getByRole("button", { name: "打开同步目录" }));

    await waitFor(() =>
      expect(mockedInvoke()).toHaveBeenCalledWith("open_download_root", {
        indexRepoPath: defaultAppSettings.indexRepoPath,
        downloadRoot: defaultAppSettings.syncRoot,
      }),
    );
    expect(await screen.findByText("已打开同步目录")).toBeTruthy();
  });

  it("retries failed sync downloads back into the sync folder", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "同步" }));
    fireEvent.click(screen.getByRole("button", { name: "扫描同步" }));
    fireEvent.click(await screen.findByRole("button", { name: "开始同步" }));
    await waitFor(() =>
      expect(mockedInvoke().mock.calls.filter(([command]) => command === "start_download")).toHaveLength(1),
    );

    await waitFor(() => expect(screen.getByRole("button", { name: "取消下载" })).toBeTruthy());
    emitDownloadProgress({
      taskId: "download-1",
      kind: "failed",
      path: "资料/数据结构/b.pdf",
      bytesWritten: 0,
      totalBytes: 2048,
      completedFiles: 0,
      failedFiles: 1,
      totalFiles: 1,
      message: "missing object",
    });
    emitDownloadProgress({
      taskId: "download-1",
      kind: "completed",
      path: null,
      bytesWritten: 0,
      totalBytes: 0,
      completedFiles: 0,
      failedFiles: 1,
      totalFiles: 1,
      message: "download task completed: 0 complete, 1 failed",
    });

    await screen.findByText("missing object");
    const retryButton = screen.getByRole("button", { name: "重试失败" }) as HTMLButtonElement;
    await waitFor(() => expect(retryButton.disabled).toBe(false));
    fireEvent.click(retryButton);

    await waitFor(() =>
      expect(mockedInvoke().mock.calls.filter(([command]) => command === "start_download")).toHaveLength(2),
    );
    const startDownloadCalls = mockedInvoke().mock.calls.filter(([command]) => command === "start_download");
    expect(startDownloadCalls[1]).toEqual([
      "start_download",
      {
        request: {
          indexRepoPath: defaultAppSettings.indexRepoPath,
          paths: ["资料/数据结构/b.pdf"],
          downloadRoot: defaultAppSettings.syncRoot,
          rclonePath: defaultAppSettings.rclonePath,
          remote: defaultAppSettings.remote,
          bucket: defaultAppSettings.bucket,
          downloadJobs: defaultAppSettings.downloadJobs,
          largeFileThresholdMiB: defaultAppSettings.largeFileThresholdMiB,
          largeFileStreams: defaultAppSettings.largeFileStreams,
          showLargeFileProgress: defaultAppSettings.showLargeFileProgress,
        },
      },
    ]);
  });

  it("sends the saved large-file progress toggle with download requests", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());

    fireEvent.click(screen.getByRole("button", { name: "设置" }));
    fireEvent.click(screen.getByLabelText("大文件下载进度展示"));
    fireEvent.click(screen.getByRole("button", { name: "资料" }));
    fireEvent.click(screen.getByRole("checkbox", { name: /资料\/数据结构\/a\.pdf/ }));
    fireEvent.click(screen.getByRole("button", { name: "开始下载" }));

    await waitFor(() =>
      expect(mockedInvoke()).toHaveBeenCalledWith("start_download", {
        request: {
          indexRepoPath: defaultAppSettings.indexRepoPath,
          paths: ["资料/数据结构/a.pdf"],
          downloadRoot: defaultAppSettings.downloadRoot,
          rclonePath: defaultAppSettings.rclonePath,
          remote: defaultAppSettings.remote,
          bucket: defaultAppSettings.bucket,
          downloadJobs: defaultAppSettings.downloadJobs,
          largeFileThresholdMiB: defaultAppSettings.largeFileThresholdMiB,
          largeFileStreams: defaultAppSettings.largeFileStreams,
          showLargeFileProgress: false,
        },
      }),
    );
  });

  it("checks the configured rclone remote from the download panel", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());

    fireEvent.click(screen.getByRole("button", { name: "检查 R2" }));

    await waitFor(() =>
      expect(mockedInvoke()).toHaveBeenCalledWith("check_rclone_remote", {
        rclonePath: defaultAppSettings.rclonePath,
        remote: defaultAppSettings.remote,
        bucket: defaultAppSettings.bucket,
      }),
    );
    expect(await screen.findByText("R2 连接正常")).toBeTruthy();
  });

  it("prepares and opens the configured download directory", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());

    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));

    await waitFor(() =>
      expect(mockedInvoke()).toHaveBeenCalledWith("open_download_root", {
        indexRepoPath: defaultAppSettings.indexRepoPath,
        downloadRoot: defaultAppSettings.downloadRoot,
      }),
    );
    expect(await screen.findByText("已打开下载目录")).toBeTruthy();
  });

  it("shows failed download rows and retries only failed paths", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());

    fireEvent.click(screen.getByRole("checkbox", { name: /资料\/数据结构\/a\.pdf/ }));
    fireEvent.click(screen.getByRole("button", { name: "开始下载" }));

    await waitFor(() =>
      expect(mockedInvoke()).toHaveBeenCalledWith("start_download", {
        request: {
          indexRepoPath: defaultAppSettings.indexRepoPath,
          paths: ["资料/数据结构/a.pdf"],
          downloadRoot: defaultAppSettings.downloadRoot,
          rclonePath: defaultAppSettings.rclonePath,
          remote: defaultAppSettings.remote,
          bucket: defaultAppSettings.bucket,
          downloadJobs: defaultAppSettings.downloadJobs,
          largeFileThresholdMiB: defaultAppSettings.largeFileThresholdMiB,
          largeFileStreams: defaultAppSettings.largeFileStreams,
          showLargeFileProgress: defaultAppSettings.showLargeFileProgress,
        },
      }),
    );
    emitDownloadProgress({
      taskId: "download-1",
      kind: "failed",
      path: "资料/数据结构/a.pdf",
      bytesWritten: 0,
      totalBytes: 1024,
      completedFiles: 0,
      failedFiles: 1,
      totalFiles: 1,
      message: "missing object",
    });
    emitDownloadProgress({
      taskId: "download-1",
      kind: "completed",
      path: null,
      bytesWritten: 0,
      totalBytes: 0,
      completedFiles: 0,
      failedFiles: 1,
      totalFiles: 1,
      message: "download task completed: 0 complete, 1 failed",
    });

    fireEvent.click(screen.getByRole("button", { name: "下载" }));

    const queue = await screen.findByLabelText("下载任务列表");
    expect(await within(queue).findByText("失败")).toBeTruthy();
    expect(await within(queue).findByText("missing object")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "重试失败" }));

    await waitFor(() =>
      expect(mockedInvoke()).toHaveBeenLastCalledWith("start_download", {
        request: {
          indexRepoPath: defaultAppSettings.indexRepoPath,
          paths: ["资料/数据结构/a.pdf"],
          downloadRoot: defaultAppSettings.downloadRoot,
          rclonePath: defaultAppSettings.rclonePath,
          remote: defaultAppSettings.remote,
          bucket: defaultAppSettings.bucket,
          downloadJobs: defaultAppSettings.downloadJobs,
          largeFileThresholdMiB: defaultAppSettings.largeFileThresholdMiB,
          largeFileStreams: defaultAppSettings.largeFileStreams,
          showLargeFileProgress: defaultAppSettings.showLargeFileProgress,
        },
      }),
    );
  });

  it("shows live byte progress and cancels the active task", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());

    fireEvent.click(screen.getByRole("checkbox", { name: /资料\/数据结构\/a\.pdf/ }));
    fireEvent.click(screen.getByRole("button", { name: "开始下载" }));

    await waitFor(() => expect(screen.getByRole("button", { name: "取消下载" })).toBeTruthy());
    emitDownloadProgress({
      taskId: "download-1",
      kind: "progress",
      path: "资料/数据结构/a.pdf",
      bytesWritten: 512,
      totalBytes: 1024,
      completedFiles: 0,
      failedFiles: 0,
      totalFiles: 1,
      message: "streaming 资料/数据结构/a.pdf",
    });

    expect(await screen.findByText("512 B / 1.000 KiB")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "取消下载" }));

    await waitFor(() =>
      expect(mockedInvoke()).toHaveBeenLastCalledWith("cancel_download", {
        taskId: "download-1",
      }),
    );
  });

  it("accepts progress events that arrive before the start command resolves", async () => {
    const startDownload = createDeferred<{ taskId: string }>();
    mockedInvoke().mockImplementation((command: string) => {
      if (command === "load_settings") {
        return Promise.resolve(defaultAppSettings);
      }
      if (command === "load_manifest") {
        return Promise.resolve(records);
      }
      if (command === "start_download") {
        return startDownload.promise;
      }
      return Promise.resolve({ stdout: "", stderr: "" });
    });
    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());

    fireEvent.click(screen.getByRole("checkbox", { name: /资料\/数据结构\/a\.pdf/ }));
    fireEvent.click(screen.getByRole("button", { name: "开始下载" }));
    emitDownloadProgress({
      taskId: "download-1",
      kind: "progress",
      path: "资料/数据结构/a.pdf",
      bytesWritten: 512,
      totalBytes: 1024,
      completedFiles: 0,
      failedFiles: 0,
      totalFiles: 1,
      message: "streaming 资料/数据结构/a.pdf",
    });

    expect(await screen.findByText("512 B / 1.000 KiB")).toBeTruthy();

    startDownload.resolve({ taskId: "download-1" });
  });

  it("keeps a stable multi-file queue on the downloads view while resources show summary progress", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());

    fireEvent.click(screen.getByRole("button", { name: "选中当前列表" }));
    fireEvent.click(screen.getByRole("button", { name: "开始下载" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "取消下载" })).toBeTruthy());

    expect(await screen.findByText("0 / 2")).toBeTruthy();
    expect(screen.queryByText("queued 资料/数据结构/a.pdf")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "下载" }));

    const queue = await screen.findByLabelText("下载任务列表");
    expect(queue.textContent).toContain("资料/数据结构/a.pdf");
    expect(queue.textContent).toContain("资料/数据结构/b.pdf");
    expect((queue.textContent ?? "").indexOf("资料/数据结构/a.pdf")).toBeLessThan(
      (queue.textContent ?? "").indexOf("资料/数据结构/b.pdf"),
    );

    emitDownloadProgress({
      taskId: "download-1",
      kind: "finished",
      path: "资料/数据结构/b.pdf",
      bytesWritten: 2048,
      totalBytes: 2048,
      completedFiles: 1,
      failedFiles: 0,
      totalFiles: 2,
      message: "downloaded 资料/数据结构/b.pdf",
    });

    await waitFor(() => expect(queue.textContent).toContain("完成"));
    expect((queue.textContent ?? "").indexOf("资料/数据结构/a.pdf")).toBeLessThan(
      (queue.textContent ?? "").indexOf("资料/数据结构/b.pdf"),
    );
    expect(queue.textContent).toContain("downloaded 资料/数据结构/b.pdf");
  });

  it("keeps large resource result sets scrollable instead of truncating them to 500 rows", async () => {
    mockedInvoke().mockImplementation((command: string) => {
      if (command === "load_settings") {
        return Promise.resolve(defaultAppSettings);
      }
      if (command === "load_manifest") {
        return Promise.resolve(Array.from({ length: 650 }, (_, index) => makeRecord(index)));
      }
      return Promise.resolve({ stdout: "", stderr: "" });
    });

    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/性能测试/file-0000.pdf")).toBeTruthy());
    expect(await screen.findByText("650 项匹配")).toBeTruthy();
    expect(screen.getByLabelText("资料列表").getAttribute("data-total-items")).toBe("650");
  });

  it("virtualizes very large download queues on the downloads view", async () => {
    const manyRecords = Array.from({ length: 1000 }, (_, index) => makeRecord(index));
    mockedInvoke().mockImplementation((command: string) => {
      if (command === "load_settings") {
        return Promise.resolve(defaultAppSettings);
      }
      if (command === "load_manifest") {
        return Promise.resolve(manyRecords);
      }
      if (command === "start_download") {
        return Promise.resolve({ taskId: "download-1" });
      }
      return Promise.resolve({ stdout: "", stderr: "" });
    });

    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/性能测试/file-0000.pdf")).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "选中当前列表" }));
    fireEvent.click(screen.getByRole("button", { name: "开始下载" }));
    fireEvent.click(screen.getByRole("button", { name: "下载" }));

    const queue = await screen.findByLabelText("下载任务列表");
    expect(queue.getAttribute("data-total-items")).toBe("1000");
    expect(screen.queryByText("资料/性能测试/file-0999.pdf")).toBeNull();
    expect(screen.getAllByText(/排队|下载中|完成|失败|取消|空文件/).length).toBeLessThan(80);
  });

  it("virtualizes large sync scan results and filters pending sync rows", async () => {
    mockedInvoke().mockImplementation((command: string) => {
      if (command === "load_settings") {
        return Promise.resolve(defaultAppSettings);
      }
      if (command === "load_manifest") {
        return Promise.resolve(records);
      }
      if (command === "scan_sync_plan") {
        return Promise.resolve(makeSyncPlan(800));
      }
      return Promise.resolve({ stdout: "", stderr: "" });
    });

    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "同步" }));
    fireEvent.click(screen.getByRole("button", { name: "扫描同步" }));

    const pending = await screen.findByLabelText("待同步文件列表");
    const extras = await screen.findByLabelText("额外本地文件列表");
    expect(pending.getAttribute("data-total-items")).toBe("800");
    expect(extras.getAttribute("data-total-items")).toBe("800");
    expect(screen.queryByText("资料/同步/file-0799.pdf")).toBeNull();

    fireEvent.change(screen.getByLabelText("同步筛选"), { target: { value: "outdated" } });
    expect(await screen.findByText("当前筛选条件下没有待同步文件。")).toBeTruthy();
  });

  it("coalesces burst progress updates and keeps the latest row state", async () => {
    const originalRequestAnimationFrame = window.requestAnimationFrame;
    const originalCancelAnimationFrame = window.cancelAnimationFrame;
    const callbacks: FrameRequestCallback[] = [];
    window.requestAnimationFrame = ((callback: FrameRequestCallback) => {
      callbacks.push(callback);
      return callbacks.length;
    }) as typeof window.requestAnimationFrame;
    window.cancelAnimationFrame = vi.fn() as typeof window.cancelAnimationFrame;

    try {
      render(<App />);

      await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());

      fireEvent.click(screen.getByRole("checkbox", { name: /资料\/数据结构\/a\.pdf/ }));
      fireEvent.click(screen.getByRole("button", { name: "开始下载" }));
      fireEvent.click(screen.getByRole("button", { name: "下载" }));

      const queue = await screen.findByLabelText("下载任务列表");
      emitDownloadProgress({
        taskId: "download-1",
        kind: "progress",
        path: "资料/数据结构/a.pdf",
        bytesWritten: 128,
        totalBytes: 1024,
        completedFiles: 0,
        failedFiles: 0,
        totalFiles: 1,
        message: "first chunk",
      });
      emitDownloadProgress({
        taskId: "download-1",
        kind: "progress",
        path: "资料/数据结构/a.pdf",
        bytesWritten: 768,
        totalBytes: 1024,
        completedFiles: 0,
        failedFiles: 0,
        totalFiles: 1,
        message: "latest chunk",
      });

      expect(callbacks).toHaveLength(1);
      expect(within(queue).queryByText("768 B / 1.000 KiB")).toBeNull();
      callbacks.splice(0).forEach((callback) => callback(16));

      expect(await within(queue).findByText("768 B / 1.000 KiB")).toBeTruthy();
      expect(await within(queue).findByText("latest chunk")).toBeTruthy();
    } finally {
      window.requestAnimationFrame = originalRequestAnimationFrame;
      window.cancelAnimationFrame = originalCancelAnimationFrame;
    }
  });
});
