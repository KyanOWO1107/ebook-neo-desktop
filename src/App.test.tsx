// @vitest-environment jsdom

import { invoke } from "@tauri-apps/api/core";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import App from "./App";
import { defaultAppSettings, type ManifestRecord } from "./manifest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
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
];

function mockedInvoke() {
  return invoke as Mock;
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
      if (command === "download_selected") {
        return Promise.resolve({
          stdout: "Downloaded 0 file(s), 1 failed.",
          stderr: "",
          items: [
            {
              path: "资料/数据结构/a.pdf",
              status: "failed",
              message: "missing object",
            },
          ],
        });
      }
      return Promise.resolve({ stdout: "", stderr: "" });
    });
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("keeps settings text inputs editable when values are cleared or pasted", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getAllByText("资料/数据结构/a.pdf").length).toBeGreaterThan(0));

    const downloadRoot = screen.getByLabelText("下载目录") as HTMLInputElement;
    const indexRepoPath = screen.getByLabelText("索引仓库") as HTMLInputElement;

    fireEvent.change(downloadRoot, { target: { value: "" } });
    expect(downloadRoot.value).toBe("");

    fireEvent.change(downloadRoot, { target: { value: "D:/TYUT downloads" } });
    expect(downloadRoot.value).toBe("D:/TYUT downloads");

    fireEvent.change(indexRepoPath, { target: { value: "" } });
    expect(indexRepoPath.value).toBe("");

    fireEvent.change(indexRepoPath, { target: { value: "E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo" } });
    expect(indexRepoPath.value).toBe("E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo");
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

    expect(await screen.findByText("失败")).toBeTruthy();
    expect(await screen.findByText("missing object")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "重试失败" }));

    await waitFor(() =>
      expect(mockedInvoke()).toHaveBeenLastCalledWith("download_selected", {
        request: {
          indexRepoPath: defaultAppSettings.indexRepoPath,
          paths: ["资料/数据结构/a.pdf"],
          downloadRoot: defaultAppSettings.downloadRoot,
          rclonePath: defaultAppSettings.rclonePath,
          remote: defaultAppSettings.remote,
          bucket: defaultAppSettings.bucket,
          downloadJobs: defaultAppSettings.downloadJobs,
        },
      }),
    );
  });
});
