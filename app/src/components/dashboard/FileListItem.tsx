import { useState, memo } from 'react';
import { Folder, Eye, HardDrive, Plus, Share2 } from 'lucide-react';
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

    return (
        <div
            onClick={(e) => onFileClick(e, file.id)}
            onContextMenu={(e) => handleContextMenu(e, file)}
            draggable={transferEnabled && !isFolder}
            onDragStart={(e) => {
                if (!transferEnabled || isFolder) {
                    e.preventDefault();
                    return;
                }
                if (onDragStart) onDragStart(file.id);
                e.dataTransfer.setData("application/x-telegram-file-id", file.id.toString());
                e.dataTransfer.effectAllowed = 'move';
            }}
            onDragEnd={() => {
                if (onDragEnd) onDragEnd();
            }}
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
            className={`group grid grid-cols-[2rem_2fr_6rem_8rem] gap-4 items-center px-4 py-3 rounded-lg cursor-pointer border border-transparent transition-all hover:bg-telegram-hover 
                ${selectedIds.includes(file.id) ? 'bg-telegram-primary/10 border-telegram-primary/20' : ''}
                ${isDragOver ? 'ring-2 ring-telegram-primary bg-telegram-primary/20' : ''}
            `}
        >
            <div className="flex justify-center">
                {isFolder ? <Folder className="w-5 h-5 text-telegram-primary" /> : <FileTypeIcon filename={file.name} className="w-5 h-5" />}
            </div>
            <div className="truncate text-sm text-telegram-text font-medium relative pr-8">
                {file.name}
                {/* List Actions */}
                <div className="absolute right-0 top-1/2 -translate-y-1/2 opacity-0 group-hover:opacity-100 flex items-center bg-telegram-surface border border-telegram-border shadow-lg rounded px-1">
                    {!isFolder && onShare && (
                        <button onClick={(e) => { e.stopPropagation(); if (shareEnabled) onShare(file); }} disabled={!shareEnabled} title={!shareEnabled ? (shareBlockedTitle || blockedTitle) : 'Share'} className="p-1 hover:text-blue-400 text-telegram-subtext disabled:opacity-40 disabled:cursor-not-allowed"><Share2 className="w-4 h-4" /></button>
                    )}
                    <button onClick={(e) => { e.stopPropagation(); if (previewEnabled) onPreview(file); }} disabled={!previewEnabled} title={!previewEnabled ? (previewBlockedTitle || blockedTitle) : (isFolder ? 'Open' : 'Preview')} className="p-1 hover:text-telegram-text text-telegram-subtext disabled:opacity-40 disabled:cursor-not-allowed"><Eye className="w-4 h-4" /></button>
                    <button onClick={(e) => { e.stopPropagation(); if (downloadEnabled) onDownload(file.id, file.name); }} disabled={!downloadEnabled} title={!downloadEnabled ? (downloadBlockedTitle || blockedTitle) : 'Download'} className="p-1 hover:text-telegram-text text-telegram-subtext disabled:opacity-40 disabled:cursor-not-allowed"><HardDrive className="w-4 h-4" /></button>
                    <button onClick={(e) => { e.stopPropagation(); if (deleteEnabled) onDelete(file.id); }} disabled={!deleteEnabled} title={!deleteEnabled ? blockedTitle : 'Delete'} className="p-1 hover:text-red-400 text-telegram-subtext disabled:opacity-40 disabled:cursor-not-allowed"><Plus className="w-4 h-4 rotate-45" /></button>
                </div>
            </div>
            <div className="text-right text-xs text-telegram-subtext truncate">{file.sizeStr}</div>
            <div className="text-right text-xs text-telegram-subtext font-mono opacity-50 truncate">{file.created_at || '-'}</div>
        </div>
    );
});
