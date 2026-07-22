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
