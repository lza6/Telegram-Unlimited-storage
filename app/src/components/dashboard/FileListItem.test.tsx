import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent, screen } from '@testing-library/react';
import { FileListItem } from './FileListItem';
import type { TelegramFile } from '../../types';

const folder: TelegramFile = {
    id: 10,
    name: 'Folder A',
    size: 0,
    sizeStr: '0 Bytes',
    type: 'folder',
};

const file: TelegramFile = {
    id: 11,
    name: 'doc.pdf',
    size: 2048,
    sizeStr: '2 KB',
    type: 'file',
};

describe('FileListItem', () => {
    const baseProps = {
        file: folder,
        selectedIds: [] as number[],
        onFileClick: vi.fn(),
        handleContextMenu: vi.fn(),
        onPreview: vi.fn(),
        onDownload: vi.fn(),
        onDelete: vi.fn(),
    };

    it('does not call onDrop on folder row when transfer is blocked', () => {
        const onDrop = vi.fn();
        const { container } = render(
            <FileListItem {...baseProps} onDrop={onDrop} transferEnabled={false} />,
        );
        fireEvent.drop(container.firstChild as Element);
        expect(onDrop).not.toHaveBeenCalled();
    });

    it('calls onDrop on folder row when transfer is enabled', () => {
        const onDrop = vi.fn();
        const { container } = render(
            <FileListItem {...baseProps} onDrop={onDrop} transferEnabled={true} />,
        );
        fireEvent.drop(container.firstChild as Element);
        expect(onDrop).toHaveBeenCalledTimes(1);
    });

    it('invokes onShare when shareEnabled without transfer', () => {
        const onShare = vi.fn();
        render(
            <FileListItem
                {...baseProps}
                file={file}
                onShare={onShare}
                transferEnabled={false}
                shareEnabled={true}
            />,
        );
        fireEvent.click(screen.getByTitle('Share'));
        expect(onShare).toHaveBeenCalledWith(file);
    });
});
