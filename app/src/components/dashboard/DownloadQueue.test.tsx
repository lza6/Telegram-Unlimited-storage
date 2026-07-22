import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { DownloadQueue } from './DownloadQueue';
import type { DownloadItem } from '../../types';

function makeItem(overrides: Partial<DownloadItem> = {}): DownloadItem {
    return {
        id: 'dl-1',
        messageId: 100,
        filename: 'movie.mp4',
        folderId: null,
        status: 'downloading',
        progress: 40,
        uploadedBytes: 400,
        totalBytes: 1000,
        speedBytesPerSec: 50,
        ...overrides,
    } as DownloadItem;
}

describe('DownloadQueue', () => {
    it('returns null when items is empty', () => {
        const { container } = render(
            <DownloadQueue
                items={[]}
                onClearFinished={vi.fn()}
                onCancelAll={vi.fn()}
                onCancelItem={vi.fn()}
                onRetryItem={vi.fn()}
            />,
        );
        expect(container.firstChild).toBeNull();
    });

    it('renders download queue with active badge', () => {
        render(
            <DownloadQueue
                items={[makeItem()]}
                onClearFinished={vi.fn()}
                onCancelAll={vi.fn()}
                onCancelItem={vi.fn()}
                onRetryItem={vi.fn()}
            />,
        );
        expect(screen.getByText('Downloads')).toBeInTheDocument();
        expect(screen.getByText('1 active')).toBeInTheDocument();
    });

    it('shows Cancel All when downloads are active', () => {
        render(
            <DownloadQueue
                items={[makeItem({ status: 'pending' })]}
                onClearFinished={vi.fn()}
                onCancelAll={vi.fn()}
                onCancelItem={vi.fn()}
                onRetryItem={vi.fn()}
            />,
        );
        expect(screen.getByText('Cancel All')).toBeInTheDocument();
    });

    it('shows error message for failed items', () => {
        render(
            <DownloadQueue
                items={[makeItem({ status: 'error', error: 'Network error' })]}
                onClearFinished={vi.fn()}
                onCancelAll={vi.fn()}
                onCancelItem={vi.fn()}
                onRetryItem={vi.fn()}
            />,
        );
        expect(screen.getByText('Network error')).toBeInTheDocument();
    });

    it('shows Clear Finished only when success items exist', () => {
        const { rerender } = render(
            <DownloadQueue
                items={[makeItem({ status: 'error', error: 'fail' })]}
                onClearFinished={vi.fn()}
                onCancelAll={vi.fn()}
                onCancelItem={vi.fn()}
                onRetryItem={vi.fn()}
            />,
        );
        expect(screen.queryByText('Clear Finished')).not.toBeInTheDocument();

        rerender(
            <DownloadQueue
                items={[makeItem({ status: 'success' })]}
                onClearFinished={vi.fn()}
                onCancelAll={vi.fn()}
                onCancelItem={vi.fn()}
                onRetryItem={vi.fn()}
            />,
        );
        expect(screen.getByText('Clear Finished')).toBeInTheDocument();
    });
});
