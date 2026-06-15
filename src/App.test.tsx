// @vitest-environment jsdom

import { invoke } from "@tauri-apps/api/core";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
      return Promise.resolve({ stdout: "", stderr: "" });
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("keeps settings text inputs editable when values are cleared or pasted", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByText("资料/数据结构/a.pdf")).toBeTruthy());

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
});
