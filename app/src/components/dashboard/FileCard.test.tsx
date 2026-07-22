import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { HTMLAttributes } from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { FileCard } from './FileCard';
import type { TelegramFile } from '../../types';

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn().mockResolvedValue(null),
}));

vi.mock('framer-motion', () => ({
    motion: {
        div: ({ children, draggable, ...rest }: HTMLAttributes<HTMLDivElement> & { draggable?: boolean }) => (
            <div data-draggable={draggable ? 'true' : 'false'} {...rest}>
                {children}
            </div>
        ),
    },
}));

class IntersectionObserverMock {
    observe = vi.fn();
    unobserve = vi.fn();
    disconnect = vi.fn();
}

beforeEach(() => {
    Object.defineProperty(window, 'IntersectionObserver', {
        writable: true,
        configurable: true,
        value: IntersectionObserverMock,
    });
});

const baseFile: TelegramFile = {
    id: 42,
    name: 'photo.png',
    size: 1024,
    sizeStr: '1 KB',
    type: 'file',
};

const folderFile: TelegramFile = {
    ...baseFile,
    id: 99,
    name: 'My Folder',
    type: 'folder',
};

describe('FileCard', () => {
    beforeEach(() => {
        vi.clearAllMocks();
    });

    it('disables drag on files when transfer is blocked', () => {
        const { container } = render(
            <FileCard
                file={baseFile}
                onDelete={vi.fn()}
                onDownload={vi.fn()}
                isSelected={false}
                transferEnabled={false}
                blockedTitle="Session not ready"
            />,
        );
        const draggable = container.querySelector('[data-draggable]');
        expect(draggable?.getAttribute('data-draggable')).toBe('false');
    });

    it('allows drag on files when transfer is enabled', () => {
        const { container } = render(
            <FileCard
                file={baseFile}
                onDelete={vi.fn()}
                onDownload={vi.fn()}
                isSelected={false}
                transferEnabled={true}
            />,
        );
        const draggable = container.querySelector('[data-draggable]');
        expect(draggable?.getAttribute('data-draggable')).toBe('true');
    });

    it('never enables drag on folder cards', () => {
        const { container } = render(
            <FileCard
                file={folderFile}
                onDelete={vi.fn()}
                onDownload={vi.fn()}
                isSelected={false}
                transferEnabled={true}
            />,
        );
        const draggable = container.querySelector('[data-draggable]');
        expect(draggable?.getAttribute('data-draggable')).toBe('false');
    });

    it('shows blocked title on action buttons when offline', () => {
        render(
            <FileCard
                file={baseFile}
                onDelete={vi.fn()}
                onDownload={vi.fn()}
                isSelected={false}
                transferEnabled={false}
                blockedTitle="Session not ready"
            />,
        );
        const titles = screen.getAllByTitle('Session not ready');
        expect(titles.length).toBeGreaterThan(0);
    });

    it('invokes onShare when share button clicked', () => {
        const onShare = vi.fn();
        render(
            <FileCard
                file={baseFile}
                onDelete={vi.fn()}
                onDownload={vi.fn()}
                onShare={onShare}
                isSelected={false}
                transferEnabled={true}
            />,
        );
        fireEvent.click(screen.getByLabelText('Share photo.png'));
        expect(onShare).toHaveBeenCalledTimes(1);
    });

    it('allows download and delete when downloadEnabled/deleteEnabled without transfer', () => {
        const onDownload = vi.fn();
        const onDelete = vi.fn();
        render(
            <FileCard
                file={baseFile}
                onDelete={onDelete}
                onDownload={onDownload}
                isSelected={false}
                transferEnabled={false}
                downloadEnabled={true}
                deleteEnabled={true}
                blockedTitle="Session not ready"
            />,
        );
        fireEvent.click(screen.getByLabelText('Download photo.png'));
        fireEvent.click(screen.getByLabelText('Delete photo.png'));
        expect(onDownload).toHaveBeenCalledTimes(1);
        expect(onDelete).toHaveBeenCalledTimes(1);
    });

    it('allows share when shareEnabled without transfer', () => {
        const onShare = vi.fn();
        render(
            <FileCard
                file={baseFile}
                onDelete={vi.fn()}
                onDownload={vi.fn()}
                onShare={onShare}
                isSelected={false}
                transferEnabled={false}
                shareEnabled={true}
                blockedTitle="Session not ready"
            />,
        );
        fireEvent.click(screen.getByLabelText('Share photo.png'));
        expect(onShare).toHaveBeenCalledTimes(1);
    });

    it('allows preview when previewEnabled without transfer', () => {
        const onPreview = vi.fn();
        render(
            <FileCard
                file={baseFile}
                onDelete={vi.fn()}
                onDownload={vi.fn()}
                onPreview={onPreview}
                isSelected={false}
                transferEnabled={false}
                previewEnabled={true}
                downloadEnabled={true}
                blockedTitle="Session not ready"
            />,
        );
        fireEvent.click(screen.getByLabelText('Preview photo.png'));
        expect(onPreview).toHaveBeenCalledTimes(1);
    });

    it('does not render share button on folders', () => {
        render(
            <FileCard
                file={folderFile}
                onDelete={vi.fn()}
                onDownload={vi.fn()}
                onShare={vi.fn()}
                isSelected={false}
                transferEnabled={true}
            />,
        );
        expect(screen.queryByLabelText('Share My Folder')).toBeNull();
    });
});
