/** Pure helpers mirrored in deploy/web/assets/web-pure.js — keep in sync. */

export function safeNext(raw: string | null | undefined, origin = 'http://localhost'): string {
    if (!raw || typeof raw !== 'string') return '/dashboard.html';
    const trimmed = raw.trim();
    if (!trimmed.startsWith('/')) return '/dashboard.html';
    try {
        const u = new URL(trimmed, origin);
        if (u.origin !== origin) return '/dashboard.html';
        const path = u.pathname;
        if (!path.startsWith('/') || path.startsWith('//')) return '/dashboard.html';
        if (path.includes('login.html')) return '/dashboard.html';
        return path + u.search + u.hash;
    } catch {
        return '/dashboard.html';
    }
}

export function safeHttpUrl(url: string, origin = 'http://localhost'): string {
    const raw = String(url).trim();
    if (!/^https?:\/\//i.test(raw)) return '#';
    try {
        const u = new URL(raw, origin);
        if (u.protocol === 'http:' || u.protocol === 'https:') {
            return u.href;
        }
    } catch {
        /* invalid */
    }
    return '#';
}

export function escapeHtml(s: string): string {
    return String(s)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

export type RebuildIndexTrigger = 'refresh' | 'search' | 'manual';

/** Background rebuild on refresh/search — toast only for explicit manual actions. */
export function rebuildIndexShouldToast(trigger: RebuildIndexTrigger): boolean {
    return trigger === 'manual';
}

export function formatRebuildIndexSuccessToast(
    filesIndexed: number,
    foldersScanned: number,
): string {
    return `索引已重建：${filesIndexed} 个文件 / ${foldersScanned} 个文件夹`;
}

/** Background refresh/search rebuild failures should be visible but non-blocking. */
export function rebuildIndexShouldSurfaceBackgroundFailure(trigger: RebuildIndexTrigger): boolean {
    return trigger === 'refresh' || trigger === 'search';
}

export function formatRebuildIndexBackgroundFailureMessage(err?: unknown): string {
    const detail = err != null ? String(err) : '';
    if (detail) {
        return `后台索引重建失败（列表仍可用）：${detail}`;
    }
    return '后台索引重建未完成，将使用实时文件扫描';
}

export function shouldShowBotOnboarding(
    transportMode: string | undefined,
    connected: boolean,
    dismissed: boolean,
): boolean {
    return transportMode === 'bot' && !connected && !dismissed;
}

export function shouldShowUserOnboarding(
    transportMode: string | undefined,
    connected: boolean,
    dismissed: boolean,
): boolean {
    return transportMode === 'user' && !connected && !dismissed;
}

export type WebHealthSnapshot = {
    ready?: boolean;
    version?: string;
    transport_mode?: string;
};

export type WebAuthSnapshot = {
    connected?: boolean;
    transport_mode?: string;
};

/** Health endpoint responded with a parseable API payload */
export function isWebApiReachable(health: WebHealthSnapshot | null | undefined): boolean {
    return !!health && typeof health.version === 'string';
}

/** Upload / download require Telegram transport ready */
export function isWebTransportReady(
    health: WebHealthSnapshot | null | undefined,
    auth: WebAuthSnapshot | null | undefined,
): boolean {
    if (!health?.ready) return false;
    if ((health.transport_mode || '').toLowerCase() === 'user') {
        return !!auth?.connected;
    }
    return true;
}

/** Share create/revoke and Bot bulk delete are DB-only — API up is enough */
export function isWebDbMutationReady(health: WebHealthSnapshot | null | undefined): boolean {
    return isWebApiReachable(health);
}

export function bulkDeleteRequiresTransport(transportMode: string | null | undefined): boolean {
    return (transportMode || '').toLowerCase() === 'user';
}

/** localStorage key — bump to notify other Web tabs that share list may be stale */
export const SHARES_INVALIDATE_STORAGE_KEY = 'td-shares-invalidate';

export function formatBulkDeleteConfirmMessage(
    count: number,
    transportMode: string | null | undefined,
): string {
    const mode = (transportMode || 'bot').toLowerCase();
    const shareNote = '相关分享链接将一并撤销。';
    if (mode === 'user') {
        return `确定删除选中的 ${count} 个文件？User 模式下将同时删除 Telegram 消息，${shareNote}`;
    }
    return `确定删除选中的 ${count} 条索引？Bot 模式下 Telegram 消息不会被删除，${shareNote}`;
}

export function formatSingleDeleteConfirmMessage(
    transportMode: string | null | undefined,
): string {
    const mode = (transportMode || 'user').toLowerCase();
    const shareNote = '相关分享链接将一并撤销。';
    if (mode === 'bot') {
        return `确定删除此文件索引？Telegram 消息不会被删除，${shareNote}`;
    }
    return `确定删除此文件？将同时删除 Telegram 消息，${shareNote}`;
}

export function formatDeleteSuccessToast(
    deletedCount: number,
    sharesRevoked?: number | null,
): string {
    if (deletedCount <= 0) return '没有可删除的条目';
    let sharePart: string;
    if (sharesRevoked != null) {
        sharePart =
            sharesRevoked > 0
                ? `，已撤销 ${sharesRevoked} 条分享链接`
                : '';
    } else {
        sharePart = '，相关分享链接已一并撤销';
    }
    if (deletedCount === 1) return `已删除 1 条${sharePart}`;
    return `已删除 ${deletedCount} 条${sharePart}`;
}
