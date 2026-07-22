import { describe, it, expect, vi } from 'vitest';
import {
    safeNext,
    safeHttpUrl,
    escapeHtml,
    rebuildIndexShouldToast,
    rebuildIndexShouldSurfaceBackgroundFailure,
    formatRebuildIndexSuccessToast,
    formatRebuildIndexBackgroundFailureMessage,
    shouldShowBotOnboarding,
    shouldShowUserOnboarding,
    isWebApiReachable,
    isWebTransportReady,
    isWebDbMutationReady,
    bulkDeleteRequiresTransport,
    SHARES_INVALIDATE_STORAGE_KEY,
    formatBulkDeleteConfirmMessage,
    formatDeleteSuccessToast,
    formatSingleDeleteConfirmMessage,
} from './webPure';

describe('safeNext', () => {
    const origin = 'http://localhost:1334';

    it('defaults to dashboard when empty', () => {
        expect(safeNext(null, origin)).toBe('/dashboard.html');
    });

    it('allows same-origin relative paths', () => {
        expect(safeNext('/files.html?q=1', origin)).toBe('/files.html?q=1');
    });

    it('blocks external origins', () => {
        expect(safeNext('https://evil.com/phish', origin)).toBe('/dashboard.html');
    });

    it('blocks login redirect loops', () => {
        expect(safeNext('/login.html?next=/x', origin)).toBe('/dashboard.html');
    });

    it('blocks protocol-relative paths', () => {
        expect(safeNext('//evil.com/x', origin)).toBe('/dashboard.html');
    });

    it('falls back on malformed input', () => {
        expect(safeNext('not a url %%', origin)).toBe('/dashboard.html');
    });

    it('returns dashboard when URL constructor throws', () => {
        const RealURL = globalThis.URL;
        vi.stubGlobal(
            'URL',
            class BadURL {
                constructor() {
                    throw new TypeError('invalid');
                }
            },
        );
        expect(safeNext('/files.html', origin)).toBe('/dashboard.html');
        vi.stubGlobal('URL', RealURL);
    });
});

describe('safeHttpUrl', () => {
    const origin = 'http://localhost:1334';

    it('allows http and https', () => {
        expect(safeHttpUrl('https://cdn.example.com/f.bin', origin)).toBe(
            'https://cdn.example.com/f.bin',
        );
    });

    it('rejects javascript scheme', () => {
        expect(safeHttpUrl('javascript:alert(1)', origin)).toBe('#');
    });

    it('rejects ftp and invalid urls', () => {
        expect(safeHttpUrl('ftp://files.example.com/a', origin)).toBe('#');
        expect(safeHttpUrl(':::bad', origin)).toBe('#');
        expect(safeHttpUrl('http://[::1', origin)).toBe('#');
    });
});

describe('escapeHtml', () => {
    it('escapes script-breaking characters', () => {
        expect(escapeHtml('</textarea><script>')).toBe('&lt;/textarea&gt;&lt;script&gt;');
    });
});

describe('index rebuild UX', () => {
    it('only manual rebuild shows toast', () => {
        expect(rebuildIndexShouldToast('refresh')).toBe(false);
        expect(rebuildIndexShouldToast('search')).toBe(false);
        expect(rebuildIndexShouldToast('manual')).toBe(true);
    });

    it('formats rebuild success toast', () => {
        expect(formatRebuildIndexSuccessToast(10, 2)).toContain('10');
        expect(formatRebuildIndexSuccessToast(10, 2)).toContain('2');
    });

    it('surfaces background failure for refresh/search only', () => {
        expect(rebuildIndexShouldSurfaceBackgroundFailure('refresh')).toBe(true);
        expect(rebuildIndexShouldSurfaceBackgroundFailure('search')).toBe(true);
        expect(rebuildIndexShouldSurfaceBackgroundFailure('manual')).toBe(false);
    });

    it('formats background failure message with optional error', () => {
        expect(formatRebuildIndexBackgroundFailureMessage()).toContain('实时');
        expect(formatRebuildIndexBackgroundFailureMessage('timeout')).toContain('timeout');
    });
});

describe('web onboarding cards', () => {
    it('shows bot card only when bot mode and not connected', () => {
        expect(shouldShowBotOnboarding('bot', false, false)).toBe(true);
        expect(shouldShowBotOnboarding('bot', true, false)).toBe(false);
        expect(shouldShowBotOnboarding('user', false, false)).toBe(false);
        expect(shouldShowBotOnboarding('bot', false, true)).toBe(false);
    });

    it('shows user card only when user mode and not connected', () => {
        expect(shouldShowUserOnboarding('user', false, false)).toBe(true);
        expect(shouldShowUserOnboarding('user', true, false)).toBe(false);
        expect(shouldShowUserOnboarding('bot', false, false)).toBe(false);
    });
});

describe('web readiness gates', () => {
    it('detects API reachability from health payload', () => {
        expect(isWebApiReachable(null)).toBe(false);
        expect(isWebApiReachable({ ready: true })).toBe(false);
        expect(isWebApiReachable({ version: '4.0.0', ready: false })).toBe(true);
    });

    it('requires user auth when transport is user', () => {
        const health = { ready: true, transport_mode: 'user', version: '4' };
        expect(isWebTransportReady(health, { connected: false })).toBe(false);
        expect(isWebTransportReady(health, { connected: true })).toBe(true);
    });

    it('allows bot transport when health.ready', () => {
        const health = { ready: true, transport_mode: 'bot', version: '4' };
        expect(isWebTransportReady(health, { connected: false })).toBe(true);
    });

    it('allows DB mutations when API reachable even if transport down', () => {
        expect(isWebDbMutationReady({ version: '4', ready: false })).toBe(true);
    });

    it('bulk delete transport requirement follows mode', () => {
        expect(bulkDeleteRequiresTransport('user')).toBe(true);
        expect(bulkDeleteRequiresTransport('bot')).toBe(false);
    });
});

describe('delete UX and cross-tab share invalidation', () => {
    it('uses stable storage key for share list refresh', () => {
        expect(SHARES_INVALIDATE_STORAGE_KEY).toBe('td-shares-invalidate');
    });

    it('confirm message mentions share revocation per mode', () => {
        expect(formatBulkDeleteConfirmMessage(3, 'user')).toContain('Telegram');
        expect(formatBulkDeleteConfirmMessage(3, 'user')).toContain('分享');
        expect(formatBulkDeleteConfirmMessage(2, 'bot')).toContain('索引');
        expect(formatBulkDeleteConfirmMessage(2, 'bot')).toContain('分享');
    });

    it('success toast mentions share revocation', () => {
        expect(formatDeleteSuccessToast(1)).toContain('分享');
        expect(formatDeleteSuccessToast(5)).toContain('5');
        expect(formatDeleteSuccessToast(0)).toContain('没有');
        expect(formatDeleteSuccessToast(2, 3)).toContain('已撤销 3 条');
        expect(formatDeleteSuccessToast(1, 0)).toBe('已删除 1 条');
    });

    it('single delete confirm mentions mode and shares', () => {
        expect(formatSingleDeleteConfirmMessage('user')).toContain('Telegram');
        expect(formatSingleDeleteConfirmMessage('bot')).toContain('索引');
        expect(formatSingleDeleteConfirmMessage('bot')).toContain('分享');
    });
});
