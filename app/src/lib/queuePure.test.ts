import { describe, expect, it } from 'vitest';
import {
    applyCancelAllTransfers,
    applyCancelTransferItem,
    applyRetryTransferItem,
    computeAvailableSlots,
    countSuccessTransfers,
    filterClearFinishedTransfers,
    hasActiveTransfers,
    hasClearableFinishedTransfers,
    selectPendingTransfers,
} from './queuePure';

type Item = { id: string; status: string; error?: string; progress?: number };

describe('queuePure', () => {
    it('computeAvailableSlots respects in-flight count', () => {
        expect(computeAvailableSlots(0, 3)).toBe(3);
        expect(computeAvailableSlots(2, 3)).toBe(1);
        expect(computeAvailableSlots(5, 3)).toBe(0);
    });

    it('selectPendingTransfers skips in-flight and caps slots', () => {
        const queue = [
            { id: 'a', status: 'pending' },
            { id: 'b', status: 'pending' },
            { id: 'c', status: 'pending' },
            { id: 'd', status: 'uploading' },
        ];
        expect(selectPendingTransfers(queue, new Set(['b']), 2).map((i) => i.id)).toEqual(['a', 'c']);
    });

    it('applyCancelAllTransfers removes pending and cancels in-progress uploads', () => {
        const queue: Item[] = [
            { id: '1', status: 'pending' },
            { id: '2', status: 'uploading' },
            { id: '3', status: 'success' },
        ];
        const { queue: next, invokeCancelIds } = applyCancelAllTransfers(queue, 'uploading');
        expect(invokeCancelIds).toEqual(['2']);
        expect(next.map((i) => i.status)).toEqual(['cancelled', 'success']);
    });

    it('applyCancelTransferItem no-ops for finished items', () => {
        const queue: Item[] = [{ id: 's', status: 'success' }];
        const result = applyCancelTransferItem(queue, 's', 'downloading');
        expect(result.queue).toEqual(queue);
        expect(result.invokeCancelId).toBeNull();
    });

    it('applyCancelTransferItem removes pending or cancels downloading', () => {
        const queue: Item[] = [
            { id: 'p', status: 'pending' },
            { id: 'd', status: 'downloading' },
        ];
        expect(applyCancelTransferItem(queue, 'p', 'downloading').queue).toHaveLength(1);
        const cancelled = applyCancelTransferItem(queue, 'd', 'downloading');
        expect(cancelled.invokeCancelId).toBe('d');
        expect(cancelled.queue.find((i) => i.id === 'd')?.status).toBe('cancelled');
    });

    it('applyRetryTransferItem resets error/cancelled to pending', () => {
        const queue: Item[] = [
            { id: 'e', status: 'error', error: 'timeout', progress: 12 },
            { id: 'c', status: 'cancelled' },
            { id: 's', status: 'success' },
        ];
        const next = applyRetryTransferItem(queue, 'e');
        expect(next[0].status).toBe('pending');
        expect(next[0].error).toBeUndefined();
        expect(next[0].progress).toBeUndefined();
        expect(applyRetryTransferItem(next, 'c')[1].status).toBe('pending');
    });

    it('filterClearFinishedTransfers keeps non-success items', () => {
        const queue: Item[] = [
            { id: '1', status: 'success' },
            { id: '2', status: 'error' },
        ];
        expect(filterClearFinishedTransfers(queue)).toEqual([{ id: '2', status: 'error' }]);
    });

    it('countSuccessTransfers and hasClearableFinishedTransfers', () => {
        const queue: Item[] = [
            { id: '1', status: 'success' },
            { id: '2', status: 'error' },
            { id: '3', status: 'success' },
        ];
        expect(countSuccessTransfers(queue)).toBe(2);
        expect(hasClearableFinishedTransfers(queue)).toBe(true);
        expect(hasClearableFinishedTransfers([{ status: 'error' }])).toBe(false);
    });

    it('hasActiveTransfers detects pending/uploading/downloading', () => {
        expect(hasActiveTransfers([{ status: 'success' }])).toBe(false);
        expect(hasActiveTransfers([{ status: 'pending' }])).toBe(true);
        expect(hasActiveTransfers([{ status: 'downloading' }])).toBe(true);
    });
});
