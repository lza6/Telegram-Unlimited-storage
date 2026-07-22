/** Pure transfer-queue helpers — shared by upload/download hooks (Vitest target). */

export type InProgressStatus = 'uploading' | 'downloading';

export function computeAvailableSlots(inFlightCount: number, maxConcurrent: number): number {
    return Math.max(0, maxConcurrent - inFlightCount);
}

export function selectPendingTransfers<T extends { id: string; status: string }>(
    queue: T[],
    inFlightIds: Iterable<string>,
    slots: number,
): T[] {
    if (slots <= 0) return [];
    const inFlight = new Set(inFlightIds);
    return queue.filter((i) => i.status === 'pending' && !inFlight.has(i.id)).slice(0, slots);
}

export function applyCancelAllTransfers<T extends { id: string; status: string }>(
    queue: T[],
    inProgressStatus: InProgressStatus,
): { queue: T[]; invokeCancelIds: string[] } {
    const invokeCancelIds = queue.filter((i) => i.status === inProgressStatus).map((i) => i.id);
    const next = queue
        .filter((i) => i.status !== 'pending')
        .map((i) =>
            i.status === inProgressStatus ? ({ ...i, status: 'cancelled' } as T) : i,
        );
    return { queue: next, invokeCancelIds };
}

export function applyCancelTransferItem<T extends { id: string; status: string }>(
    queue: T[],
    id: string,
    inProgressStatus: InProgressStatus,
): { queue: T[]; invokeCancelId: string | null } {
    const item = queue.find((i) => i.id === id);
    if (!item) return { queue, invokeCancelId: null };
    if (item.status === inProgressStatus) {
        return {
            queue: queue.map((i) =>
                i.id === id ? ({ ...i, status: 'cancelled' } as T) : i,
            ),
            invokeCancelId: id,
        };
    }
    if (item.status === 'pending') {
        return { queue: queue.filter((i) => i.id !== id), invokeCancelId: null };
    }
    return { queue, invokeCancelId: null };
}

export function applyRetryTransferItem<
    T extends {
        id: string;
        status: string;
        error?: string;
        progress?: number;
        uploadedBytes?: number;
        totalBytes?: number;
        speedBytesPerSec?: number;
    },
>(queue: T[], id: string): T[] {
    return queue.map((i) =>
        i.id === id && (i.status === 'error' || i.status === 'cancelled')
            ? {
                  ...i,
                  status: 'pending' as const,
                  error: undefined,
                  progress: undefined,
                  uploadedBytes: undefined,
                  totalBytes: undefined,
                  speedBytesPerSec: undefined,
              }
            : i,
    );
}

export function filterClearFinishedTransfers<T extends { status: string }>(queue: T[]): T[] {
    return queue.filter((i) => i.status !== 'success');
}

export function countSuccessTransfers(queue: { status: string }[]): number {
    return queue.filter((i) => i.status === 'success').length;
}

export function hasClearableFinishedTransfers(queue: { status: string }[]): boolean {
    return countSuccessTransfers(queue) > 0;
}

export function hasActiveTransfers(
    queue: { status: string }[],
    activeStatuses: readonly string[] = ['pending', 'uploading', 'downloading'],
): boolean {
    return queue.some((i) => activeStatuses.includes(i.status));
}
