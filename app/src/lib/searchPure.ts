/** Global search helpers — keep Web/desktop index rebuild behavior testable. */

export const GLOBAL_SEARCH_MIN_LEN = 3;

export function isGlobalSearchActive(term: string, minLen = GLOBAL_SEARCH_MIN_LEN): boolean {
    return term.trim().length >= minLen;
}

/** Rebuild once when entering global search (not on every debounced keystroke). */
export function shouldRebuildIndexForGlobalSearch(
    wasActive: boolean,
    term: string,
    minLen = GLOBAL_SEARCH_MIN_LEN,
): boolean {
    return isGlobalSearchActive(term, minLen) && !wasActive;
}

/** Bot / authoritative index: search DB directly — GramJS rebuild needs User session. */
export function shouldRebuildIndexBeforeGlobalSearch(opts: {
    botIndexMode: boolean;
    wasActive: boolean;
    term: string;
    minLen?: number;
}): boolean {
    if (opts.botIndexMode) return false;
    return shouldRebuildIndexForGlobalSearch(
        opts.wasActive,
        opts.term,
        opts.minLen ?? GLOBAL_SEARCH_MIN_LEN,
    );
}

export function buildRebuildFolderIds(folders: { id: number }[]): (number | null)[] {
    return [null, ...folders.map((f) => f.id)];
}

/** Non-fatal index rebuild failure before global search — user should know scan may be slower. */
export function formatIndexRebuildBackgroundFailureMessage(err?: unknown): string {
    const detail = err != null ? String(err) : '';
    if (detail) {
        return `索引重建失败，将使用实时搜索：${detail}`;
    }
    return '索引重建未完成，将使用实时 Telegram 搜索';
}
