import { describe, expect, it, vi } from "vitest";
import {
  buildUploadQueueEntries,
  classifyUploadFailure,
  computeChunkPlan,
  formatSize,
  newUploadSessionId,
  parseUploadFolderId,
} from "./uploadPure";

describe("uploadPure", () => {
  it("formatSize scales units", () => {
    expect(formatSize(512)).toBe("512 B");
    expect(formatSize(2048)).toBe("2.00 KB");
    expect(formatSize(3 * 1024 * 1024)).toBe("3.00 MB");
    expect(formatSize(2 * 1024 * 1024 * 1024)).toBe("2.00 GB");
  });

  it("computeChunkPlan splits file", () => {
    const plan = computeChunkPlan(25 * 1024 * 1024, 10 * 1024 * 1024);
    expect(plan.chunkCount).toBe(3);
    expect(plan.chunkBytes).toBe(10 * 1024 * 1024);
  });

  it("newUploadSessionId returns non-empty unique ids", () => {
    const a = newUploadSessionId();
    const b = newUploadSessionId();
    expect(a.length).toBeGreaterThan(8);
    expect(b.length).toBeGreaterThan(8);
    expect(a).not.toBe(b);
  });

  it("newUploadSessionId falls back when crypto.randomUUID missing", () => {
    const original = globalThis.crypto;
    vi.stubGlobal("crypto", undefined);
    const id = newUploadSessionId();
    expect(id.startsWith("sess-")).toBe(true);
    vi.stubGlobal("crypto", original);
  });

  it("parseUploadFolderId treats empty as Saved Messages", () => {
    expect(parseUploadFolderId("")).toBeNull();
    expect(parseUploadFolderId("12")).toBe(12);
    expect(parseUploadFolderId("abc")).toBeNull();
  });

  it("buildUploadQueueEntries tags active folder", () => {
    expect(buildUploadQueueEntries(["/a.txt", "/b.txt"], 7)).toEqual([
      { path: "/a.txt", folderId: 7, status: "pending" },
      { path: "/b.txt", folderId: 7, status: "pending" },
    ]);
  });

  it("computeChunkPlan uses default chunk size when zero", () => {
    const plan = computeChunkPlan(1024, 0);
    expect(plan.chunkBytes).toBe(20 * 1024 * 1024);
    expect(plan.chunkCount).toBe(1);
  });
});

describe("classifyUploadFailure", () => {
  const isSessionLost = (msg: string) => msg.includes("SESSION_LOST");

  it("classifies cancelled transfers", () => {
    expect(classifyUploadFailure("Transfer cancelled")).toBe("cancelled");
  });

  it("classifies file size errors", () => {
    expect(classifyUploadFailure("FILE_TOO_BIG")).toBe("file_too_big");
    expect(classifyUploadFailure("exceeds 2 GB")).toBe("file_too_big");
  });

  it("classifies session loss via callback", () => {
    expect(classifyUploadFailure("SESSION_LOST", { isSessionLost })).toBe("session_lost");
  });

  it("defaults to generic", () => {
    expect(classifyUploadFailure("network timeout")).toBe("generic");
  });
});
