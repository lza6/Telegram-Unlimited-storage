import { describe, it, expect } from 'vitest';
import {
    GLOBAL_SEARCH_MIN_LEN,
    isGlobalSearchActive,
    shouldRebuildIndexBeforeGlobalSearch,
    shouldRebuildIndexForGlobalSearch,
    buildRebuildFolderIds,
    formatIndexRebuildBackgroundFailureMessage,
} from './searchPure';

describe('isGlobalSearchActive', () => {
    it('requires at least GLOBAL_SEARCH_MIN_LEN characters', () => {
        expect(GLOBAL_SEARCH_MIN_LEN).toBe(3);
        expect(isGlobalSearchActive('ab')).toBe(false);
        expect(isGlobalSearchActive('abc')).toBe(true);
    });
});

describe('shouldRebuildIndexForGlobalSearch', () => {
    it('rebuilds only when first entering global search', () => {
        expect(shouldRebuildIndexForGlobalSearch(false, 'photos')).toBe(true);
        expect(shouldRebuildIndexForGlobalSearch(true, 'photos')).toBe(false);
        expect(shouldRebuildIndexForGlobalSearch(false, 'ab')).toBe(false);
    });
});

describe('shouldRebuildIndexBeforeGlobalSearch', () => {
    it('skips rebuild in bot index mode', () => {
        expect(
            shouldRebuildIndexBeforeGlobalSearch({
                botIndexMode: true,
                wasActive: false,
                term: 'photos',
            }),
        ).toBe(false);
    });

    it('rebuilds in user mode when first entering search', () => {
        expect(
            shouldRebuildIndexBeforeGlobalSearch({
                botIndexMode: false,
                wasActive: false,
                term: 'photos',
            }),
        ).toBe(true);
    });
});

describe('buildRebuildFolderIds', () => {
    it('includes Saved Messages and all folder ids', () => {
        expect(buildRebuildFolderIds([{ id: 10 }, { id: 20 }])).toEqual([null, 10, 20]);
        expect(buildRebuildFolderIds([])).toEqual([null]);
    });
});

describe('formatIndexRebuildBackgroundFailureMessage', () => {
    it('includes error detail when provided', () => {
        expect(formatIndexRebuildBackgroundFailureMessage('network')).toContain('network');
    });

    it('falls back to generic scan message', () => {
        expect(formatIndexRebuildBackgroundFailureMessage()).toContain('实时');
    });
});
