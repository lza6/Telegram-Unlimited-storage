import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { describeUploadState, UploadQueue } from '../../components/dashboard/UploadQueue';
import type { QueueItem } from '../../types';

// Mock QueueItem data factory
function makeItem(overrides: Partial<QueueItem> = {}): QueueItem {
    return {
        id: 'test-id-1',
        path: '/test/file.mp4',
        status: 'uploading',
        progress: 50,
        uploadedBytes: 524288,
        totalBytes: 1048576,
        speedBytesPerSec: 102400,
        ...overrides,
    } as QueueItem;
}

describe('UploadQueue', () => {
    it('returns null when items is empty', () => {
        const { container } = render(
            <UploadQueue items={[]} onClearFinished={vi.fn()} onCancelAll={vi.fn()} onCancelItem={vi.fn()} onRetryItem={vi.fn()} />
        );
        expect(container.firstChild).toBeNull();
    });

    it('renders upload queue region with items', () => {
        const items = [makeItem()];
        render(
            <UploadQueue items={items} onClearFinished={vi.fn()} onCancelAll={vi.fn()} onCancelItem={vi.fn()} onRetryItem={vi.fn()} />
        );
        expect(screen.getByRole('region', { name: 'Upload queue' })).toBeInTheDocument();
        expect(screen.getByText('Uploads')).toBeInTheDocument();
    });

    it('shows file name from path', () => {
        const items = [makeItem({ path: '/videos/movie.mp4' })];
        render(
            <UploadQueue items={items} onClearFinished={vi.fn()} onCancelAll={vi.fn()} onCancelItem={vi.fn()} onRetryItem={vi.fn()} />
        );
        expect(screen.getByText('movie.mp4')).toBeInTheDocument();
    });

    it('shows Cancel All when items have pending or uploading status', () => {
        const items = [makeItem({ status: 'uploading' })];
        render(
            <UploadQueue items={items} onClearFinished={vi.fn()} onCancelAll={vi.fn()} onCancelItem={vi.fn()} onRetryItem={vi.fn()} />
        );
        expect(screen.getByText('Cancel All')).toBeInTheDocument();
    });

    it('hides Cancel All when all items are done/error/cancelled', () => {
        const items = [makeItem({ status: 'success' })];
        render(
            <UploadQueue items={items} onClearFinished={vi.fn()} onCancelAll={vi.fn()} onCancelItem={vi.fn()} onRetryItem={vi.fn()} />
        );
        expect(screen.queryByText('Cancel All')).not.toBeInTheDocument();
    });

    it('shows error message for failed items', () => {
        const items = [makeItem({ status: 'error', error: 'Upload timed out' })];
        render(
            <UploadQueue items={items} onClearFinished={vi.fn()} onCancelAll={vi.fn()} onCancelItem={vi.fn()} onRetryItem={vi.fn()} />
        );
        expect(screen.getByText('Upload timed out')).toBeInTheDocument();
    });

    it('shows Clear Finished only when success items exist', () => {
        const onlyError = [makeItem({ status: 'error', error: 'fail' })];
        const { rerender } = render(
            <UploadQueue items={onlyError} onClearFinished={vi.fn()} onCancelAll={vi.fn()} onCancelItem={vi.fn()} onRetryItem={vi.fn()} />
        );
        expect(screen.queryByText('Clear Finished')).not.toBeInTheDocument();

        rerender(
            <UploadQueue items={[makeItem({ status: 'success' })]} onClearFinished={vi.fn()} onCancelAll={vi.fn()} onCancelItem={vi.fn()} onRetryItem={vi.fn()} />
        );
        expect(screen.getByText('Clear Finished')).toBeInTheDocument();
    });

    it('shows Cancelled label for cancelled items', () => {
        const items = [makeItem({ status: 'cancelled' })];
        render(
            <UploadQueue items={items} onClearFinished={vi.fn()} onCancelAll={vi.fn()} onCancelItem={vi.fn()} onRetryItem={vi.fn()} />
        );
        expect(screen.getByText('Cancelled')).toBeInTheDocument();
    });
    it.each([
        ['UPLOAD_IN_PROGRESS', '同一上传正在处理中'],
        ['UPLOAD_RECONCILIATION_REQUIRED', '正在等待数据库对账'],
        ['UPLOAD_COMPENSATION_PENDING', '等待补偿处理'],
        ['MANUAL_REVIEW', '需要人工审查'],
        ['SCHEDULER_COOLDOWN', '排队或限流冷却'],
    ])('maps backend state %s to actionable text', (error, expected) => {
        expect(describeUploadState(makeItem({ status: 'error', error }))).toContain(expected);
    });

    it('announces retry-safe attention state and keeps Windows file name', () => {
        render(
            <UploadQueue
                items={[makeItem({ path: 'C:\\uploads\\movie.mp4', status: 'error', error: 'UPLOAD_RECONCILIATION_REQUIRED' })]}
                onClearFinished={vi.fn()}
                onCancelAll={vi.fn()}
                onCancelItem={vi.fn()}
                onRetryItem={vi.fn()}
            />,
        );
        expect(screen.getByText('movie.mp4')).toBeInTheDocument();
        expect(screen.getByRole('status')).toHaveTextContent('复用原任务标识');
        expect(screen.getByRole('img', { name: /数据库对账/ })).toBeInTheDocument();
    });
});