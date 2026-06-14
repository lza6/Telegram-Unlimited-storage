import { describe, it, expect } from 'vitest';
import {
    joinDirFile,
    resolvePathSeparator,
    buildBulkDownloadItems,
    shouldBlockDuplicateDownload,
    computeDownloadPercent,
    formatDownloadProgressLabel,
    deriveWebDownloadButtonState,
    consumeStreamWithProgress,
    parseContentLengthHeader,
    resolveBlobDownloadFilename,
    buildDownloadStartToast,
    classifyDownloadFailure,
    canEnqueueDownload,
    buildLocalApiDownloadUrl,
} from './downloadPure';
import { isSessionLostError } from '../utils/sessionError';

describe('resolvePathSeparator', () => {
    it('uses backslash on Windows-style paths', () => {
        expect(resolvePathSeparator('C:\\Users\\dl')).toBe('\\');
    });

    it('uses slash on posix paths', () => {
        expect(resolvePathSeparator('/home/dl')).toBe('/');
    });
});

describe('joinDirFile', () => {
    it('joins without duplicating separator', () => {
        expect(joinDirFile('/tmp/', 'a.txt')).toBe('/tmp/a.txt');
        expect(joinDirFile('/tmp', 'a.txt')).toBe('/tmp/a.txt');
        expect(joinDirFile('C:\\dl\\', 'b.zip')).toBe('C:\\dl\\b.zip');
    });
});

describe('buildBulkDownloadItems', () => {
    it('maps files to download queue entries with folder fallback', () => {
        const items = buildBulkDownloadItems(
            [{ id: 10, name: 'a.png', folder_id: 5 }, { id: 11, name: 'b.pdf' }],
            '/downloads',
            99,
        );
        expect(items).toEqual([
            { messageId: 10, filename: 'a.png', folderId: 5, savePath: '/downloads/a.png' },
            { messageId: 11, filename: 'b.pdf', folderId: 99, savePath: '/downloads/b.pdf' },
        ]);
    });
});

describe('Web blob download UX', () => {
    it('shouldBlockDuplicateDownload detects in-flight ids', () => {
        const set = new Set(['42']);
        expect(shouldBlockDuplicateDownload(set, 42)).toBe(true);
        expect(shouldBlockDuplicateDownload(set, '99')).toBe(false);
    });

    it('computeDownloadPercent handles known total and indeterminate', () => {
        expect(computeDownloadPercent(50, 200)).toBe(25);
        expect(computeDownloadPercent(200, 200)).toBe(100);
        expect(computeDownloadPercent(10, 0)).toBeNull();
        expect(computeDownloadPercent(10, null)).toBeNull();
    });

    it('formatDownloadProgressLabel shows percent when known', () => {
        expect(formatDownloadProgressLabel(45)).toBe('下载中 45%');
        expect(formatDownloadProgressLabel(null)).toBe('下载中…');
    });

    it('deriveWebDownloadButtonState toggles label with optional percent', () => {
        expect(deriveWebDownloadButtonState(true)).toEqual({ label: '下载中…', inFlight: true });
        expect(deriveWebDownloadButtonState(true, 45)).toEqual({ label: '下载中 45%', inFlight: true });
        expect(deriveWebDownloadButtonState(false)).toEqual({ label: '下载', inFlight: false });
    });

    it('parseContentLengthHeader parses valid header', () => {
        expect(parseContentLengthHeader('1024')).toBe(1024);
        expect(parseContentLengthHeader('')).toBeNull();
        expect(parseContentLengthHeader('abc')).toBeNull();
    });

    it('consumeStreamWithProgress reports incremental percent', async () => {
        const chunks = [new Uint8Array(40), new Uint8Array(60)];
        let i = 0;
        const progress: (number | null)[] = [];
        const result = await consumeStreamWithProgress(
            {
                read: async () => {
                    if (i >= chunks.length) return { done: true };
                    const value = chunks[i++];
                    return { done: false, value };
                },
            },
            100,
            (p) => progress.push(p),
        );
        expect(result).toHaveLength(2);
        expect(progress).toContain(40);
        expect(progress[progress.length - 1]).toBe(100);
    });

    it('resolveBlobDownloadFilename prefers name then filename', () => {
        expect(resolveBlobDownloadFilename({ name: 'a.txt' })).toBe('a.txt');
        expect(resolveBlobDownloadFilename({ filename: 'b.zip' })).toBe('b.zip');
        expect(resolveBlobDownloadFilename(null)).toBe('download');
    });

    it('buildDownloadStartToast wraps filename', () => {
        expect(buildDownloadStartToast({ name: 'clip.mp4' })).toBe('正在下载「clip.mp4」…');
    });
});

describe('canEnqueueDownload', () => {
    it('allows when GramJS transfer ready', () => {
        expect(canEnqueueDownload({ transferReady: true })).toBe(true);
        expect(canEnqueueDownload({ transferReady: true, botIndexReady: false })).toBe(true);
    });

    it('allows Bot index without User session', () => {
        expect(canEnqueueDownload({ transferReady: false, botIndexReady: true })).toBe(true);
    });

    it('blocks when neither path ready', () => {
        expect(canEnqueueDownload({ transferReady: false })).toBe(false);
        expect(canEnqueueDownload({ transferReady: false, botIndexReady: false })).toBe(false);
    });
});

describe('buildLocalApiDownloadUrl', () => {
    it('builds download path with optional folder_id', () => {
        expect(buildLocalApiDownloadUrl({ port: 8080, messageId: 42 })).toBe(
            'http://127.0.0.1:8080/api/v1/files/42/download',
        );
        expect(buildLocalApiDownloadUrl({ port: 9090, messageId: 7, folderId: 100 })).toBe(
            'http://127.0.0.1:9090/api/v1/files/7/download?folder_id=100',
        );
    });
});

describe('classifyDownloadFailure', () => {
    it('detects cancelled transfers', () => {
        expect(classifyDownloadFailure('Transfer cancelled')).toBe('cancelled');
    });

    it('detects session lost via callback', () => {
        expect(
            classifyDownloadFailure('session expired', { isSessionLost: isSessionLostError }),
        ).toBe('session_lost');
    });

    it('falls back to generic', () => {
        expect(classifyDownloadFailure('disk full')).toBe('generic');
    });
});
