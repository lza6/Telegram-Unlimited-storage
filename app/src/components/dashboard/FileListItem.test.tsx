import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { FileListItem } from './FileListItem';

const file = {
    id: 12,
    name: 'report.pdf',
    size: 1024,
    sizeStr: '1 KB',
    type: 'file' as const,
    folder_id: null,
};

describe('FileListItem accessibility', () => {
    it('exposes keyboard activation and labelled action controls', () => {
        const onFileClick = vi.fn();
        const onPreview = vi.fn();
        const onDownload = vi.fn();
        const onDelete = vi.fn();
        const onShare = vi.fn();

        render(
            <FileListItem
                file={file}
                selectedIds={[file.id]}
                onFileClick={onFileClick}
                handleContextMenu={vi.fn()}
                onPreview={onPreview}
                onDownload={onDownload}
                onDelete={onDelete}
                onShare={onShare}
            />,
        );

        const item = screen.getByRole('button', { name: /report.pdf, selected/i });
        item.focus();
        fireEvent.keyDown(item, { key: 'Enter' });
        fireEvent.keyDown(item, { key: ' ' });

        expect(onFileClick).toHaveBeenCalledTimes(2);
        expect(screen.getByRole('button', { name: 'Share report.pdf' })).toBeVisible();
        expect(screen.getByRole('button', { name: 'Preview report.pdf' })).toBeVisible();
        expect(screen.getByRole('button', { name: 'Download report.pdf' })).toBeVisible();
        expect(screen.getByRole('button', { name: 'Delete report.pdf' })).toBeVisible();
    });
});
