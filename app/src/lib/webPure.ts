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
