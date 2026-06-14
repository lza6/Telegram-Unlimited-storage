import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { TransferQueuePanel } from './TransferQueuePanel';

describe('TransferQueuePanel', () => {
    it('returns null when empty', () => {
        const { container } = render(
            <TransferQueuePanel
                items={[]}
                panelClassName="panel"
                title="Transfers"
                activeStatuses={['pending', 'uploading']}
                inProgressStatus="uploading"
                progressBarClassName="bg-blue-500"
                getItemLabel={(item) => (item as { label: string }).label}
                renderStatusIndicator={() => <span data-testid="dot" />}
                onClearFinished={vi.fn()}
                onCancelAll={vi.fn()}
                onCancelItem={vi.fn()}
                onRetryItem={vi.fn()}
            />,
        );
        expect(container.firstChild).toBeNull();
    });

    it('renders shared queue chrome and progress row', () => {
        render(
            <TransferQueuePanel
                items={[
                    {
                        id: '1',
                        status: 'uploading',
                        label: 'clip.mp4',
                        progress: 40,
                        uploadedBytes: 400,
                        totalBytes: 1000,
                    },
                ]}
                panelClassName="panel"
                ariaLabel="Transfer queue"
                title="Uploads"
                activeStatuses={['pending', 'uploading']}
                inProgressStatus="uploading"
                progressBarClassName="bg-blue-500"
                getItemLabel={(item) => (item as { label: string }).label}
                renderStatusIndicator={() => <span data-testid="dot" />}
                onClearFinished={vi.fn()}
                onCancelAll={vi.fn()}
                onCancelItem={vi.fn()}
                onRetryItem={vi.fn()}
            />,
        );
        expect(screen.getByRole('region', { name: 'Transfer queue' })).toBeInTheDocument();
        expect(screen.getByText('Uploads')).toBeInTheDocument();
        expect(screen.getByText('clip.mp4')).toBeInTheDocument();
        expect(screen.getByText('400.0 B / 1000.0 B')).toBeInTheDocument();
        expect(screen.getByText('Cancel All')).toBeInTheDocument();
    });
});
