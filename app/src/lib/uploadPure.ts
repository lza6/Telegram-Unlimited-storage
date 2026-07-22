/** Pure helpers mirrored by deploy/web/assets/upload-core.js (Vitest target). */

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function computeChunkPlan(totalBytes: number, chunkBytes: number) {
  const size = chunkBytes > 0 ? chunkBytes : 20 * 1024 * 1024;
  return { chunkCount: Math.max(1, Math.ceil(totalBytes / size)), chunkBytes: size };
}

/** Mirrors deploy/web/assets/upload-core.js — one session_id per chunked upload. */
export function newUploadSessionId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `sess-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

/** Saved Messages when empty; invalid values become null (omit field). */
export function parseUploadFolderId(raw: string | null | undefined): number | null {
  if (!raw || !String(raw).trim()) return null;
  const n = parseInt(String(raw).trim(), 10);
  return Number.isFinite(n) && n > 0 ? n : null;
}

export function buildUploadQueueEntries(
  paths: string[],
  folderId: number | null,
): Array<{ path: string; folderId: number | null; status: 'pending' }> {
  return paths.map((path) => ({ path, folderId, status: 'pending' as const }));
}

export type UploadFailureKind = 'cancelled' | 'file_too_big' | 'session_lost' | 'generic';

/** Classify upload invoke errors — shared by desktop hook (Vitest target). */
export function classifyUploadFailure(
  errMsg: string,
  options?: { isSessionLost?: (msg: string) => boolean },
): UploadFailureKind {
  if (errMsg.includes('Transfer cancelled')) return 'cancelled';
  if (
    errMsg.includes('FILE_TOO_BIG') ||
    errMsg.includes('too large') ||
    errMsg.includes('2 GB')
  ) {
    return 'file_too_big';
  }
  if (options?.isSessionLost?.(errMsg)) return 'session_lost';
  return 'generic';
}
