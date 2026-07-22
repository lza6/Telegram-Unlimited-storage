import { KeyboardEvent, memo, useState } from 'react';
import { Eye, Folder, HardDrive, Plus, Share2 } from 'lucide-react';
import { TelegramFile } from '../../types';
import { FileTypeIcon } from '../FileTypeIcon';

interface FileListItemProps {
    file: TelegramFile;
    selectedIds: number[];
    onFileClick: (e: React.MouseEvent, id: number) => void;
    handleContextMenu: (e: React.MouseEvent, file: TelegramFile) => void;
    onDragStart?: (fileId: number) => void;
    onDragEnd?: () => void;
    onDrop?: (e: React.DragEvent, folderId: number) => void;
    onPreview: (file: TelegramFile) => void;
    onDownload: (id: number, name: string) => void;
    onDelete: (id: number) => void;
    onShare?: (file: TelegramFile) => void;
    transferEnabled?: boolean;
    previewEnabled?: boolean;
    downloadEnabled?: boolean;
    shareEnabled?: boolean;
    deleteEnabled?: boolean;
    blockedTitle?: string;
    downloadBlockedTitle?: string;
    previewBlockedTitle?: string;
    shareBlockedTitle?: string;
}

export const FileListItem = memo(function FileListItem({
    file, selectedIds, onFileClick, handleContextMenu,
    onDragStart, onDragEnd, onDrop,
    onPreview, onDownload, onDelete, onShare,
    transferEnabled = true,
    previewEnabled = transferEnabled,
    downloadEnabled = transferEnabled,
    shareEnabled = transferEnabled,
    deleteEnabled = transferEnabled,
    blockedTitle,
    downloadBlockedTitle,
    previewBlockedTitle,
    shareBlockedTitle,
}: FileListItemProps) {
    const [isDragOver, setIsDragOver] = useState(false);
    const isFolder = file.type === 'folder';
    const isSelected = selectedIds.includes(file.id);
    const itemLabel = `${file.name}${isSelected ? ', selected' : ''}`;

    const activateItem = (event: KeyboardEvent<HTMLDivElement>) => {
        if (event.key !== 'Enter' && event.key !== ' ') return;
        event.preventDefault();
        onFileClick(event as unknown as React.MouseEvent, file.id);
    };

    return (
        <div
            role="button"
            tabIndex={0}
            aria-label={itemLabel}
            aria-pressed={isSelected}
            onKeyDown={activateItem}
            onClick={(e) => onFileClick(e, file.id)}
            onContextMenu={(e) => handleContextMenu(e, file)}
            draggable={transferEnabled && !isFolder}
            onDragStart={(e) => {
                if (!transferEnabled || isFolder) {
                    e.preventDefault();
                    return;
                }
                onDragStart?.(file.id);
                e.dataTransfer.setData('application/x-telegram-file-id', file.id.toString());
                e.dataTransfer.effectAllowed = 'move';
            }}
            onDragEnd={() => onDragEnd?.()}
            onDragOver={(e) => {
                if (isFolder && transferEnabled) {
                    e.preventDefault();
                    e.stopPropagation();
                    if (!isDragOver) setIsDragOver(true);
                }
            }}
            onDragLeave={(e) => {
                if (isFolder && transferEnabled) {
                    e.preventDefault();
                    e.stopPropagation();
                    setIsDragOver(false);
                }
            }}
            onDrop={(e) => {
                if (isFolder && onDrop && transferEnabled) {
                    e.preventDefault();
                    e.stopPropagation();
                    setIsDragOver(false);
                    onDrop(e, file.id);
                }
            }}
            className={`group grid grid-cols-[2rem_2fr_6rem_8rem] items-center gap-4 rounded-lg border border-transparent px-4 py-3 transition-all hover:bg-telegram-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-telegram-primary
                ${isSelected ? 'bg-telegram-primary/10 border-telegram-primary/20' : ''}
                ${isDragOver ? 'ring-2 ring-telegram-primary bg-telegram-primary/20' : ''}
            `}
        >
            <div className="flex justify-center" aria-hidden="true">
                {isFolder ? <Folder className="h-5 w-5 text-telegram-primary" /> : <FileTypeIcon filename={file.name} className="h-5 w-5" />}
            </div>
            <div className="relative truncate pr-8 text-sm font-medium text-telegram-text">
                {file.name}
                <div className="absolute right-0 top-1/2 flex -translate-y-1/2 items-center rounded border border-telegram-border bg-telegram-surface px-1 opacity-0 shadow-lg transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
                    {!isFolder && onShare && (
                        <button type="button" aria-label={`Share ${file.name}`} onClick={(e) => { e.stopPropagation(); if (shareEnabled) onShare(file); }} disabled={!shareEnabled} title={!shareEnabled ? (shareBlockedTitle || blockedTitle) : 'Share'} className="p-1 text-telegram-subtext hover:text-blue-400 disabled:cursor-not-allowed disabled:opacity-40"><Share2 className="h-4 w-4" /></button>
                    )}
                    <button type="button" aria-label={`${isFolder ? 'Open' : 'Preview'} ${file.name}`} onClick={(e) => { e.stopPropagation(); if (previewEnabled) onPreview(file); }} disabled={!previewEnabled} title={!previewEnabled ? (previewBlockedTitle || blockedTitle) : (isFolder ? 'Open' : 'Preview')} className="p-1 text-telegram-subtext hover:text-telegram-text disabled:cursor-not-allowed disabled:opacity-40"><Eye className="h-4 w-4" /></button>
                    <button type="button" aria-label={`Download ${file.name}`} onClick={(e) => { e.stopPropagation(); if (downloadEnabled) onDownload(file.id, file.name); }} disabled={!downloadEnabled} title={!downloadEnabled ? (downloadBlockedTitle || blockedTitle) : 'Download'} className="p-1 text-telegram-subtext hover:text-telegram-text disabled:cursor-not-allowed disabled:opacity-40"><HardDrive className="h-4 w-4" /></button>
                    <button type="button" aria-label={`Delete ${file.name}`} onClick={(e) => { e.stopPropagation(); if (deleteEnabled) onDelete(file.id); }} disabled={!deleteEnabled} title={!deleteEnabled ? blockedTitle : 'Delete'} className="p-1 text-telegram-subtext hover:text-red-400 disabled:cursor-not-allowed disabled:opacity-40"><Plus className="h-4 w-4 rotate-45" /></button>
                </div>
            </div>
            <div className="truncate text-right text-xs text-telegram-subtext">{file.sizeStr}</div>
            <div className="truncate text-right font-mono text-xs text-telegram-subtext opacity-50">{file.created_at || '-'}</div>
        </div>
    );
});
