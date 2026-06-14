export function formatBytes(bytes: number, decimals = 2) {
    if (!+bytes) return '0 Bytes';
    const k = 1024;
    const dm = decimals < 0 ? 0 : decimals;
    const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(dm))} ${sizes[i]}`;
}

// ── File type classification ────────────────────────────────────────────

const VIDEO_EXTENSIONS = ['mp4', 'webm', 'ogg', 'mov', 'mkv', 'avi'] as const;
const AUDIO_EXTENSIONS = ['mp3', 'wav', 'aac', 'flac', 'm4a', 'opus'] as const;
const MEDIA_EXTENSIONS: readonly string[] = [...VIDEO_EXTENSIONS, ...AUDIO_EXTENSIONS];
const IMAGE_EXTENSIONS = ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp', 'svg', 'heic', 'heif'] as const;

const endsWithAny = (name: string, exts: readonly string[]) => {
    const lower = name.toLowerCase();
    return exts.some(ext => lower.endsWith(ext));
};

export const isMediaFile   = (name: string) => endsWithAny(name, MEDIA_EXTENSIONS);
export const isVideoFile   = (name: string) => endsWithAny(name, VIDEO_EXTENSIONS);
export const isAudioFile   = (name: string) => endsWithAny(name, AUDIO_EXTENSIONS);
export const isImageFile   = (name: string) => endsWithAny(name, IMAGE_EXTENSIONS);
export const isPdfFile     = (name: string) => name.toLowerCase().endsWith('.pdf');

/** Telegram peer for a file: prefer indexed folder_id, else current folder. */
export function resolveFileFolderId(
    file: { folder_id?: number | null },
    activeFolderId: number | null,
): number | null {
    if (file.folder_id !== undefined && file.folder_id !== null) {
        return file.folder_id;
    }
    return activeFolderId;
}

/** Returns true if file lives in the given folder (uses resolveFileFolderId). */
export function fileBelongsToFolder(
    file: { folder_id?: number | null } | null | undefined,
    folderId: number | null,
    activeFolderId: number | null,
): boolean {
    if (!file) return false;
    return resolveFileFolderId(file, activeFolderId) === folderId;
}

export function idsIncludeOpenFile(
    ids: number[],
    openFile: { id: number } | null | undefined,
): boolean {
    if (!openFile) return false;
    return ids.includes(openFile.id);
}

/** Remove files whose ids were deleted/moved (Telegram assigns new ids on move). */
export function pruneSelectedIdsAfterDelete(
    selectedIds: number[],
    removedIds: number[],
): number[] {
    if (removedIds.length === 0) return selectedIds;
    const removed = new Set(removedIds);
    return selectedIds.filter((id) => !removed.has(id));
}

export function filterFilesExcludingIds<T extends { id: number }>(
    files: T[],
    excludedIds: number[],
): T[] {
    if (excludedIds.length === 0) return files;
    const drop = new Set(excludedIds);
    return files.filter((f) => !drop.has(f.id));
}

export type MoveFilesPayload = {
    oldIds: number[];
    newIds: number[];
    targetFolderId: number | null;
};

export function moveResultToPayload(result: {
    oldMessageIds: number[];
    newMessageIds: number[];
    targetFolderId: number | null;
}): MoveFilesPayload {
    return {
        oldIds: result.oldMessageIds,
        newIds: result.newMessageIds,
        targetFolderId: result.targetFolderId,
    };
}

export function remapMovedFilesInList<T extends { id: number; folder_id?: number | null }>(
    files: T[],
    payload: MoveFilesPayload,
): T[] {
    const idMap = new Map<number, number>();
    payload.oldIds.forEach((oldId, index) => {
        const newId = payload.newIds[index];
        if (newId != null) idMap.set(oldId, newId);
    });
    if (idMap.size === 0) {
        return filterFilesExcludingIds(files, payload.oldIds);
    }
    return files.map((file) => {
        const mapped = idMap.get(file.id);
        if (mapped == null) return file;
        return { ...file, id: mapped, folder_id: payload.targetFolderId };
    });
}

export function mergeMovePayloads(payloads: MoveFilesPayload[]): MoveFilesPayload | null {
    if (payloads.length === 0) return null;
    const merged: MoveFilesPayload = {
        oldIds: [],
        newIds: [],
        targetFolderId: payloads[payloads.length - 1].targetFolderId,
    };
    for (const payload of payloads) {
        merged.oldIds.push(...payload.oldIds);
        merged.newIds.push(...payload.newIds);
    }
    return merged;
}

export function remapOpenFileAfterMove<T extends { id: number; folder_id?: number | null }>(
    file: T | null | undefined,
    payload: MoveFilesPayload,
): T | null {
    if (!file) return null;
    const index = payload.oldIds.indexOf(file.id);
    if (index === -1) return file;
    const newId = payload.newIds[index];
    if (newId == null) return null;
    return { ...file, id: newId, folder_id: payload.targetFolderId };
}

export function planMoveGroups(
    ids: number[],
    files: { id: number; folder_id?: number | null }[],
    activeFolderId: number | null,
    targetFolderId: number | null,
): { sourceFolderId: number | null; ids: number[] }[] {
    const groups = groupIdsBySourceFolder(ids, files, activeFolderId);
    return Array.from(groups.values()).filter(
        (group) => group.sourceFolderId !== targetFolderId,
    );
}

export function groupIdsBySourceFolder(
    ids: number[],
    files: { id: number; folder_id?: number | null }[],
    activeFolderId: number | null,
): Map<string, { sourceFolderId: number | null; ids: number[] }> {
    const fileMap = new Map(files.map((f) => [f.id, f]));
    const groups = new Map<string, { sourceFolderId: number | null; ids: number[] }>();
    for (const id of ids) {
        const file = fileMap.get(id);
        const source = resolveFileFolderId(file ?? {}, activeFolderId);
        const key = source === null ? '__home__' : String(source);
        if (!groups.has(key)) {
            groups.set(key, { sourceFolderId: source, ids: [] });
        }
        groups.get(key)!.ids.push(id);
    }
    return groups;
}
