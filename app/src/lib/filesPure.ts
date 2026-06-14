/** Pure helpers mirrored in deploy/web/assets/files-pure.js — keep in sync. */

import { groupIdsBySourceFolder } from '../utils';

export type BulkDeletePayload = {
    action: 'delete';
    file_ids: number[];
    folder_id?: number;
};

export type BulkMovePayload = {
    action: 'move';
    file_ids: number[];
    folder_id?: number;
    payload: { folder_id: number | null };
};

export function buildBulkDeletePayloads(
    ids: number[],
    files: { id: number; folder_id?: number | null }[],
): BulkDeletePayload[] {
    const groups = groupIdsBySourceFolder(ids, files, null);
    return Array.from(groups.values()).map(({ sourceFolderId, ids: fileIds }) => {
        const body: BulkDeletePayload = { action: 'delete', file_ids: fileIds };
        if (sourceFolderId != null) {
            body.folder_id = sourceFolderId;
        }
        return body;
    });
}

export function buildBulkMovePayloads(
    ids: number[],
    files: { id: number; folder_id?: number | null }[],
    targetFolderId: number | null,
): BulkMovePayload[] {
    const groups = groupIdsBySourceFolder(ids, files, null);
    const payloads: BulkMovePayload[] = [];
    for (const { sourceFolderId, ids: fileIds } of groups.values()) {
        if (sourceFolderId === targetFolderId) continue;
        const body: BulkMovePayload = {
            action: 'move',
            file_ids: fileIds,
            payload: { folder_id: targetFolderId },
        };
        if (sourceFolderId != null) {
            body.folder_id = sourceFolderId;
        }
        payloads.push(body);
    }
    return payloads;
}

export function buildFileDownloadUrl(
    id: number | string,
    folderId: number | null | undefined,
): string {
    let url = '/api/v1/files/' + encodeURIComponent(String(id)) + '/download';
    if (folderId != null) {
        url += '?folder_id=' + encodeURIComponent(String(folderId));
    }
    return url;
}

/** Bulk move requires User transport (forward + delete); Bot mode must use desktop or switch mode. */
export function canBulkMoveInTransportMode(transportMode: string | null | undefined): boolean {
    return (transportMode || '').toLowerCase() === 'user';
}

export function bulkMoveBlockedMessage(
    transportMode: string | null | undefined,
    surface: 'web' | 'desktop' = 'web',
): string {
    if (canBulkMoveInTransportMode(transportMode)) return '';
    if (surface === 'desktop') {
        return '批量移动需要 User 模式 — 请在设置中切换传输模式。';
    }
    return 'Bulk move requires User mode — switch transport in Settings or use the desktop app.';
}

export type BulkBatchSelectionResult = {
    succeededIds: number[];
    partialBatch: boolean;
};

/**
 * Map bulk API `count` to selection IDs we may safely deselect.
 * When count < fileIds.length the backend does not say which IDs succeeded (Bot index delete).
 */
export function resolveBulkBatchSucceededIds(
    fileIds: number[],
    reportedCount: number,
): BulkBatchSelectionResult {
    const expected = fileIds.length;
    const count = Math.max(0, reportedCount);
    if (count === 0) {
        return { succeededIds: [], partialBatch: false };
    }
    if (count === expected) {
        return { succeededIds: [...fileIds], partialBatch: false };
    }
    return { succeededIds: [], partialBatch: true };
}

/** Prefer API `succeeded_ids`; fall back to count-based inference (R51 compat). */
export function pickBulkSucceededIds(
    fileIds: number[],
    reportedCount: number,
    apiSucceededIds?: number[] | null,
): BulkBatchSelectionResult {
    if (Array.isArray(apiSucceededIds) && apiSucceededIds.length > 0) {
        return { succeededIds: [...apiSucceededIds], partialBatch: false };
    }
    return resolveBulkBatchSucceededIds(fileIds, reportedCount);
}

export function buildTelegramLoginUrl(webBaseUrl: string, nextPath: string): string {
    const base = webBaseUrl.replace(/\/$/, '');
    return base + '/telegram.html?next=' + encodeURIComponent(nextPath);
}

/** Default Headless web port for User login redirect from desktop Settings. */
export const DEFAULT_HEADLESS_WEB_PORT = 1334;

export function defaultHeadlessTelegramLoginUrl(nextPath = '/settings.html'): string {
    return buildTelegramLoginUrl(`http://127.0.0.1:${DEFAULT_HEADLESS_WEB_PORT}`, nextPath);
}

export function buildDesktopApiTelegramLoginUrl(apiPort: number, nextPath = '/settings.html'): string {
    return buildTelegramLoginUrl(`http://127.0.0.1:${apiPort}`, nextPath);
}

export type TelegramLoginSurface = 'desktop_api' | 'headless';

/** Build login URLs for desktop REST vs Headless (caller probes availability). */
export function buildTelegramLoginCandidates(
    apiPort: number,
    nextPath = '/settings.html',
    headlessPort = DEFAULT_HEADLESS_WEB_PORT,
): { surface: TelegramLoginSurface; url: string }[] {
    return [
        { surface: 'desktop_api', url: buildDesktopApiTelegramLoginUrl(apiPort, nextPath) },
        {
            surface: 'headless',
            url: buildTelegramLoginUrl(`http://127.0.0.1:${headlessPort}`, nextPath),
        },
    ];
}
