import { describe, it, expect } from 'vitest';
import {
    buildBulkDeletePayloads,
    buildBulkMovePayloads,
    buildDesktopApiTelegramLoginUrl,
    buildFileDownloadUrl,
    buildTelegramLoginCandidates,
    buildTelegramLoginUrl,
    bulkMoveBlockedMessage,
    canBulkMoveInTransportMode,
    defaultHeadlessTelegramLoginUrl,
    pickBulkSucceededIds,
    resolveBulkBatchSucceededIds,
} from './filesPure';

describe('buildBulkDeletePayloads', () => {
    const files = [
        { id: 1, folder_id: 10 },
        { id: 2, folder_id: null },
        { id: 3, folder_id: 10 },
        { id: 4, folder_id: 20 },
    ];

    it('groups delete requests by folder_id', () => {
        const payloads = buildBulkDeletePayloads([1, 2, 3, 4], files);
        expect(payloads).toHaveLength(3);

        const home = payloads.find((p) => p.file_ids.includes(2));
        expect(home).toEqual({ action: 'delete', file_ids: [2] });
        expect(home).not.toHaveProperty('folder_id');

        const f10 = payloads.find((p) => p.folder_id === 10);
        expect(f10?.file_ids.sort()).toEqual([1, 3]);

        const f20 = payloads.find((p) => p.folder_id === 20);
        expect(f20?.file_ids).toEqual([4]);
    });

    it('treats missing file as Saved Messages (null folder)', () => {
        const payloads = buildBulkDeletePayloads([99], files);
        expect(payloads).toEqual([{ action: 'delete', file_ids: [99] }]);
    });
});

describe('buildBulkMovePayloads', () => {
    const files = [
        { id: 1, folder_id: 10 },
        { id: 2, folder_id: null },
        { id: 3, folder_id: 10 },
        { id: 4, folder_id: 20 },
    ];

    it('groups move requests by source folder and skips already-in-target', () => {
        const payloads = buildBulkMovePayloads([1, 2, 3, 4], files, 10);
        expect(payloads).toHaveLength(2);

        const home = payloads.find((p) => p.file_ids.includes(2));
        expect(home).toEqual({
            action: 'move',
            file_ids: [2],
            payload: { folder_id: 10 },
        });

        const f20 = payloads.find((p) => p.folder_id === 20);
        expect(f20?.file_ids).toEqual([4]);
        expect(f20?.payload).toEqual({ folder_id: 10 });
    });

    it('returns empty when all files already in target', () => {
        expect(buildBulkMovePayloads([1, 3], files, 10)).toEqual([]);
    });
});

describe('buildFileDownloadUrl', () => {
    it('omits query for Saved Messages', () => {
        expect(buildFileDownloadUrl(42, null)).toBe('/api/v1/files/42/download');
        expect(buildFileDownloadUrl(42, undefined)).toBe('/api/v1/files/42/download');
    });

    it('appends folder_id query when set', () => {
        expect(buildFileDownloadUrl('99', 7)).toBe('/api/v1/files/99/download?folder_id=7');
    });
});

describe('buildTelegramLoginUrl', () => {
    it('builds headless login url with encoded next', () => {
        expect(buildTelegramLoginUrl('http://127.0.0.1:1334', '/settings.html')).toBe(
            'http://127.0.0.1:1334/telegram.html?next=%2Fsettings.html',
        );
    });
});

describe('buildDesktopApiTelegramLoginUrl', () => {
    it('uses desktop REST port', () => {
        expect(buildDesktopApiTelegramLoginUrl(8550, '/settings.html')).toBe(
            'http://127.0.0.1:8550/telegram.html?next=%2Fsettings.html',
        );
    });
});

describe('defaultHeadlessTelegramLoginUrl', () => {
    it('points at default headless port', () => {
        expect(defaultHeadlessTelegramLoginUrl()).toContain('127.0.0.1:1334/telegram.html');
    });

    it('buildDesktopApiTelegramLoginUrl uses api port', () => {
        expect(buildDesktopApiTelegramLoginUrl(8550)).toContain('127.0.0.1:8550/telegram.html');
    });
});

describe('buildTelegramLoginCandidates', () => {
    it('prefers desktop API before headless', () => {
        const list = buildTelegramLoginCandidates(8550);
        expect(list[0].surface).toBe('desktop_api');
        expect(list[1].surface).toBe('headless');
        expect(list[0].url).toContain(':8550/telegram.html');
        expect(list[1].url).toContain(':1334/telegram.html');
    });
});

describe('resolveBulkBatchSucceededIds', () => {
    it('returns all ids when count matches batch size', () => {
        expect(resolveBulkBatchSucceededIds([1, 2, 3], 3)).toEqual({
            succeededIds: [1, 2, 3],
            partialBatch: false,
        });
    });

    it('returns empty ids and partialBatch when count is lower than batch size', () => {
        expect(resolveBulkBatchSucceededIds([1, 2, 3], 2)).toEqual({
            succeededIds: [],
            partialBatch: true,
        });
    });

    it('returns empty when count is zero', () => {
        expect(resolveBulkBatchSucceededIds([1, 2], 0)).toEqual({
            succeededIds: [],
            partialBatch: false,
        });
    });
});

describe('pickBulkSucceededIds', () => {
    it('prefers API succeeded_ids over count inference', () => {
        expect(pickBulkSucceededIds([1, 2, 3], 2, [1, 3])).toEqual({
            succeededIds: [1, 3],
            partialBatch: false,
        });
    });

    it('falls back to count inference when succeeded_ids absent', () => {
        expect(pickBulkSucceededIds([1, 2], 2, undefined)).toEqual({
            succeededIds: [1, 2],
            partialBatch: false,
        });
        expect(pickBulkSucceededIds([1, 2, 3], 2, [])).toEqual({
            succeededIds: [],
            partialBatch: true,
        });
    });
});

describe('canBulkMoveInTransportMode', () => {
    it('allows move only in user mode', () => {
        expect(canBulkMoveInTransportMode('user')).toBe(true);
        expect(canBulkMoveInTransportMode('User')).toBe(true);
        expect(canBulkMoveInTransportMode('bot')).toBe(false);
        expect(canBulkMoveInTransportMode(null)).toBe(false);
    });

    it('returns blocked message for non-user modes', () => {
        expect(bulkMoveBlockedMessage('bot')).toContain('User mode');
        expect(bulkMoveBlockedMessage('bot', 'desktop')).toContain('User 模式');
        expect(bulkMoveBlockedMessage('user')).toBe('');
    });
});
