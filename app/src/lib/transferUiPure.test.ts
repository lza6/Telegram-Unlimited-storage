import { describe, expect, it } from 'vitest';
import {
    deriveStreamUiPhase,
    formatTransferBytes,
    formatTransferProgressLabel,
    streamStatusMessage,
} from './transferUiPure';

describe('formatTransferBytes', () => {
    it('formats common sizes', () => {
        expect(formatTransferBytes(512)).toBe('512.0 B');
        expect(formatTransferBytes(2048)).toBe('2.0 KB');
    });
});

describe('formatTransferProgressLabel', () => {
    it('prefers byte range over percent', () => {
        expect(formatTransferProgressLabel(50, 500, 1000)).toBe('500.0 B / 1000.0 B');
    });

    it('falls back to percent', () => {
        expect(formatTransferProgressLabel(42, undefined, undefined)).toBe('42%');
    });

    it('returns empty when no metrics', () => {
        expect(formatTransferProgressLabel(undefined, undefined, undefined)).toBe('');
    });
});

describe('deriveStreamUiPhase', () => {
    it('orders error over loading', () => {
        expect(
            deriveStreamUiPhase({ streamError: 'fail', streamUrl: null, isBuffering: false }),
        ).toBe('error');
    });

    it('shows buffering when url ready but waiting', () => {
        expect(
            deriveStreamUiPhase({
                streamError: null,
                streamUrl: 'http://127.0.0.1/stream',
                isBuffering: true,
            }),
        ).toBe('buffering');
    });

    it('ready when url available and not buffering', () => {
        expect(
            deriveStreamUiPhase({
                streamError: null,
                streamUrl: 'http://127.0.0.1/stream',
                isBuffering: false,
            }),
        ).toBe('ready');
    });
});

describe('streamStatusMessage', () => {
    it('returns user-facing labels for non-ready phases', () => {
        expect(streamStatusMessage('loading')).toBe('Preparing stream…');
        expect(streamStatusMessage('buffering')).toBe('Buffering…');
        expect(streamStatusMessage('ready')).toBeNull();
        expect(streamStatusMessage('error')).toBeNull();
    });
});
