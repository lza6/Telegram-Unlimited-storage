/** Shared UI helpers for upload/download queues (Vitest target). */

export function formatTransferBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

export function formatTransferProgressLabel(
    progress: number | undefined,
    uploadedBytes: number | undefined,
    totalBytes: number | undefined,
): string {
    if (uploadedBytes !== undefined && totalBytes !== undefined) {
        return `${formatTransferBytes(uploadedBytes)} / ${formatTransferBytes(totalBytes)}`;
    }
    if (progress !== undefined) {
        return `${progress}%`;
    }
    return '';
}

export type StreamUiPhase = 'loading' | 'buffering' | 'ready' | 'error';

export function deriveStreamUiPhase(opts: {
    streamError: string | null;
    streamUrl: string | null;
    isBuffering: boolean;
}): StreamUiPhase {
    if (opts.streamError) return 'error';
    if (!opts.streamUrl) return 'loading';
    if (opts.isBuffering) return 'buffering';
    return 'ready';
}

export function streamStatusMessage(phase: StreamUiPhase): string | null {
    switch (phase) {
        case 'loading':
            return 'Preparing stream…';
        case 'buffering':
            return 'Buffering…';
        case 'error':
        case 'ready':
        default:
            return null;
    }
}
