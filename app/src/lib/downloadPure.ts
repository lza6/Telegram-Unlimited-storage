/** Pure download helpers — Vitest target; keep logic out of hooks. */

export function resolvePathSeparator(dirPath: string): string {
    return dirPath.includes('\\') ? '\\' : '/';
}

export function joinDirFile(dirPath: string, filename: string): string {
    const separator = resolvePathSeparator(dirPath);
    const base = dirPath.endsWith(separator) ? dirPath : `${dirPath}${separator}`;
    return `${base}${filename}`;
}

export function buildBulkDownloadItems(
    files: { id: number; name: string; folder_id?: number | null }[],
    dirPath: string,
    defaultFolderId: number | null,
): Array<{ messageId: number; filename: string; folderId: number | null; savePath: string }> {
    return files.map((file) => ({
        messageId: file.id,
        filename: file.name,
        folderId: file.folder_id ?? defaultFolderId,
        savePath: joinDirFile(dirPath, file.name),
    }));
}

/** Mirrors deploy/web/assets/download-pure.js — Web single-file blob download UX. */
export function shouldBlockDuplicateDownload(
    inFlightIds: ReadonlySet<string>,
    fileId: string | number,
): boolean {
    return inFlightIds.has(String(fileId));
}

/** Desktop/Bot: allow enqueue when GramJS transfer or Bot index stream is ready. */
export function canEnqueueDownload(opts: {
    transferReady: boolean;
    botIndexReady?: boolean;
}): boolean {
    if (opts.transferReady) return true;
    return opts.botIndexReady === true;
}

/** Local REST download URL — mirrors Rust `download_file_via_local_api` (Vitest target). */
export function buildLocalApiDownloadUrl(opts: {
    port: number;
    messageId: number;
    folderId?: number | null;
}): string {
    let url = `http://127.0.0.1:${opts.port}/api/v1/files/${opts.messageId}/download`;
    if (opts.folderId != null) {
        url += `?folder_id=${opts.folderId}`;
    }
    return url;
}

export type WebDownloadButtonState = { label: string; inFlight: boolean };

/** Percent 0–100 when Content-Length known; null when indeterminate. */
export function computeDownloadPercent(
    bytesRead: number,
    totalBytes: number | null | undefined,
): number | null {
    if (totalBytes == null || totalBytes <= 0) return null;
    const pct = Math.floor((bytesRead / totalBytes) * 100);
    return Math.min(100, Math.max(0, pct));
}

export function formatDownloadProgressLabel(percent: number | null | undefined): string {
    if (percent == null) return '下载中…';
    return `下载中 ${percent}%`;
}

export function deriveWebDownloadButtonState(
    inFlight: boolean,
    percent?: number | null,
): WebDownloadButtonState {
    if (inFlight) {
        return { label: formatDownloadProgressLabel(percent ?? null), inFlight: true };
    }
    return { label: '下载', inFlight: false };
}

export type StreamChunkReader = {
    read(): Promise<{ done: boolean; value?: Uint8Array }>;
};

/** Consume a fetch body reader and report progress — Vitest uses a mock reader. */
export async function consumeStreamWithProgress(
    reader: StreamChunkReader,
    totalBytes: number | null | undefined,
    onProgress?: (percent: number | null) => void,
): Promise<Uint8Array[]> {
    const chunks: Uint8Array[] = [];
    let received = 0;
    for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        if (value && value.length > 0) {
            chunks.push(value);
            received += value.length;
            onProgress?.(computeDownloadPercent(received, totalBytes));
        }
    }
    onProgress?.(computeDownloadPercent(received, totalBytes) ?? 100);
    return chunks;
}

export function parseContentLengthHeader(
    header: string | null | undefined,
): number | null {
    if (!header) return null;
    const n = Number.parseInt(header, 10);
    return Number.isFinite(n) && n > 0 ? n : null;
}

export function resolveBlobDownloadFilename(
    file: { name?: string; filename?: string } | null | undefined,
): string {
    if (!file) return 'download';
    return file.name || file.filename || 'download';
}

export function buildDownloadStartToast(
    file: { name?: string; filename?: string } | null | undefined,
): string {
    return `正在下载「${resolveBlobDownloadFilename(file)}」…`;
}

export type DownloadFailureKind = 'cancelled' | 'session_lost' | 'generic';

/** Classify download invoke errors — shared by desktop hook (Vitest target). */
export function classifyDownloadFailure(
    errMsg: string,
    options?: { isSessionLost?: (msg: string) => boolean },
): DownloadFailureKind {
    if (errMsg.includes('Transfer cancelled')) return 'cancelled';
    if (options?.isSessionLost?.(errMsg)) return 'session_lost';
    return 'generic';
}
