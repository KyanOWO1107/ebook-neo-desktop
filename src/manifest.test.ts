import { describe, expect, it } from "vitest";
import {
  defaultAppSettings,
  buildFolderSummaries,
  buildVisibleFolderSummaries,
  clampDownloadJobs,
  clampLargeFileStreams,
  clampLargeFileThresholdMiB,
  filterRecords,
  formatBytes,
  recordsForFolderSelection,
  mergeAppSettings,
  themeAttribute,
  summarizeRecords,
  type ManifestRecord,
} from "./manifest";

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
    path: "资料/数据结构/实验/b.docx",
    objectKey: "objects/sha256/bb/b.docx",
    sha256: "b".repeat(64),
    size: 2048,
    storage: "r2",
    updatedAt: "2026-06-12",
    visibility: "private",
  },
  {
    path: "课件/大学物理/c.pptx",
    objectKey: "objects/sha256/cc/c.pptx",
    sha256: "c".repeat(64),
    size: 4096,
    storage: "r2",
    updatedAt: "2026-06-12",
    visibility: "private",
  },
];

describe("manifest helpers", () => {
  it("summarizes file count, bytes, and roots", () => {
    expect(summarizeRecords(records)).toEqual({
      files: 3,
      bytes: 7168,
      roots: 2,
    });
  });

  it("builds top-level and second-level folder summaries for the sidebar", () => {
    expect(buildFolderSummaries(records)).toEqual([
      { path: "资料", label: "资料", files: 2, bytes: 3072, depth: 0 },
      { path: "资料/数据结构", label: "数据结构", files: 2, bytes: 3072, depth: 1 },
      { path: "课件", label: "课件", files: 1, bytes: 4096, depth: 0 },
      { path: "课件/大学物理", label: "大学物理", files: 1, bytes: 4096, depth: 1 },
    ]);
  });

  it("only shows second-level folder summaries under expanded roots", () => {
    const folders = buildFolderSummaries(records);

    expect(buildVisibleFolderSummaries(folders, new Set()).map((folder) => folder.path)).toEqual(["资料", "课件"]);
    expect(buildVisibleFolderSummaries(folders, new Set(["资料"])).map((folder) => folder.path)).toEqual([
      "资料",
      "资料/数据结构",
      "课件",
    ]);
  });

  it("filters records by case-insensitive path text", () => {
    expect(filterRecords(records, "DOCX").map((record) => record.path)).toEqual([
      "资料/数据结构/实验/b.docx",
    ]);
  });

  it("selects all matching records under the active folder", () => {
    expect(recordsForFolderSelection(records, "资料/数据结构", "").map((record) => record.path)).toEqual([
      "资料/数据结构/a.pdf",
      "资料/数据结构/实验/b.docx",
    ]);

    expect(recordsForFolderSelection(records, "资料/数据结构", "docx").map((record) => record.path)).toEqual([
      "资料/数据结构/实验/b.docx",
    ]);
  });

  it("formats bytes for compact UI display", () => {
    expect(formatBytes(1024)).toBe("1.000 KiB");
    expect(formatBytes(4096 * 1024)).toBe("4.000 MiB");
  });

  it("merges saved app settings with defaults", () => {
    expect(
      mergeAppSettings({
        downloadRoot: "D:/TYUT",
        indexRepoPath: "E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo",
        downloadJobs: 8,
        theme: "dark",
      }),
    ).toEqual({
      ...defaultAppSettings,
      downloadRoot: "D:/TYUT",
      indexRepoPath: "E:/Workplace/LR/Ebook/TYUT-ebooks-collection-neo",
      downloadJobs: 8,
      theme: "dark",
    });
  });

  it("defaults to the sibling neo index repository path", () => {
    expect(defaultAppSettings.indexRepoPath).toBe("../TYUT-ebooks-collection-neo");
  });

  it("does not include a Python command in app settings", () => {
    expect(Object.keys(defaultAppSettings)).not.toContain("pythonCommand");
  });

  it("guards theme values for DOM attributes", () => {
    expect(themeAttribute("dark")).toBe("dark");
    expect(themeAttribute("blue")).toBe("light");
  });

  it("clamps download jobs to the supported GUI range", () => {
    expect(clampDownloadJobs(0)).toBe(1);
    expect(clampDownloadJobs(8)).toBe(8);
    expect(clampDownloadJobs(99)).toBe(16);
  });

  it("defaults and clamps large file download settings", () => {
    expect(defaultAppSettings.largeFileThresholdMiB).toBe(20);
    expect(defaultAppSettings.largeFileStreams).toBe(8);
    expect(clampLargeFileThresholdMiB(0)).toBe(1);
    expect(clampLargeFileThresholdMiB(64)).toBe(64);
    expect(clampLargeFileThresholdMiB(9000)).toBe(4096);
    expect(clampLargeFileStreams(0)).toBe(1);
    expect(clampLargeFileStreams(8)).toBe(8);
    expect(clampLargeFileStreams(99)).toBe(16);
    expect(mergeAppSettings({ largeFileThresholdMiB: 0, largeFileStreams: 99 })).toMatchObject({
      largeFileThresholdMiB: 1,
      largeFileStreams: 16,
    });
  });
});
