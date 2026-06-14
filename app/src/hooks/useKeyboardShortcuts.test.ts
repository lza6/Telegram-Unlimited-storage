import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useKeyboardShortcuts } from './useKeyboardShortcuts';

function fireKey(key: string, opts: { ctrlKey?: boolean } = {}) {
    window.dispatchEvent(
        new KeyboardEvent('keydown', {
            key,
            bubbles: true,
            ctrlKey: opts.ctrlKey ?? false,
        }),
    );
}

describe('useKeyboardShortcuts', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    afterEach(() => {
        vi.restoreAllMocks();
    });

    it('Enter preview works with previewEnabled without transfer', () => {
        const onEnter = vi.fn();
        renderHook(() =>
            useKeyboardShortcuts({
                onSelectAll: vi.fn(),
                onDelete: vi.fn(),
                onEscape: vi.fn(),
                onSearch: vi.fn(),
                onEnter,
                transferEnabled: false,
                previewEnabled: true,
                deleteEnabled: false,
            }),
        );
        fireKey('Enter');
        expect(onEnter).toHaveBeenCalledTimes(1);
    });

    it('Enter blocked when previewEnabled false', () => {
        const onEnter = vi.fn();
        renderHook(() =>
            useKeyboardShortcuts({
                onSelectAll: vi.fn(),
                onDelete: vi.fn(),
                onEscape: vi.fn(),
                onSearch: vi.fn(),
                onEnter,
                transferEnabled: false,
                previewEnabled: false,
            }),
        );
        fireKey('Enter');
        expect(onEnter).not.toHaveBeenCalled();
    });

    it('Delete works with deleteEnabled without transfer', () => {
        const onDelete = vi.fn();
        renderHook(() =>
            useKeyboardShortcuts({
                onSelectAll: vi.fn(),
                onDelete,
                onEscape: vi.fn(),
                onSearch: vi.fn(),
                transferEnabled: false,
                deleteEnabled: true,
                previewEnabled: false,
            }),
        );
        fireKey('Delete');
        expect(onDelete).toHaveBeenCalledTimes(1);
    });
});
