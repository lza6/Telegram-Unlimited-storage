import { useEffect, useLayoutEffect, useRef, useState, useCallback } from 'react';
import { Eye, HardDrive, Trash2, FolderOpen, Play, FileText, Link, Copy } from 'lucide-react';
import { TelegramFile, TelegramFolder } from '../../types';
import { isMediaFile, isPdfFile } from '../../utils';
import { toast } from 'sonner';

interface ContextMenuProps {
    x: number;
    y: number;
    file: TelegramFile;
    onClose: () => void;
    onDownload: () => void;
    onDelete: () => void;
    onPreview: () => void;
    onShare?: () => void;
    folders?: TelegramFolder[];
    activeFolderId?: number | null;
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

export function ContextMenu({
    x, y, file, onClose, onDownload, onDelete, onPreview, onShare, folders, activeFolderId,
    transferEnabled = true,
    previewEnabled = transferEnabled,
    downloadEnabled = transferEnabled,
    shareEnabled = transferEnabled,
    deleteEnabled = transferEnabled,
    blockedTitle,
    downloadBlockedTitle,
    previewBlockedTitle,
    shareBlockedTitle,
}: ContextMenuProps) {
    const [adjustedPos, setAdjustedPos] = useState({ x, y });
    const menuRef = useRef<HTMLDivElement>(null);
    const itemRefs = useRef<HTMLButtonElement[]>([]);

    // Collect all enabled menu items
    const menuItems: { action: () => void; enabled: boolean; label: string }[] = [];

    if (file.type !== 'folder') {
        menuItems.push({ action: onPreview, enabled: previewEnabled, label: 'Preview' });
    }
    if (file.type === 'folder') {
        menuItems.push({ action: onPreview, enabled: true, label: 'Open' });
    }
    menuItems.push({ action: onDownload, enabled: downloadEnabled, label: 'Download' });
    if (file.type !== 'folder' && onShare) {
        menuItems.push({ action: onShare, enabled: shareEnabled, label: 'Share' });
    }
    // Copy link item - always enabled if available
    const folder = folders?.find(f => f.id === file.folder_id) || folders?.find(f => f.id === activeFolderId);
    const username = folder?.username || (folder as any)?.chat?.username || (folder as any)?.channel?.username;
    if (file.type !== 'folder' && username) {
        menuItems.push({
            action: async () => {
                const url = `https://t.me/${username}/${file.id}`;
                try {
                    await navigator.clipboard.writeText(url);
                    toast.success("Telegram 链接已复制");
                } catch {
                    toast.error("复制链接失败");
                }
                onClose();
            },
            enabled: true,
            label: 'Copy Telegram Link'
        });
    }
    menuItems.push({ action: onDelete, enabled: deleteEnabled, label: 'Delete' });

    // Adjust position to stay in bounds
    useLayoutEffect(() => {
        if (menuRef.current) {
            const rect = menuRef.current.getBoundingClientRect();
            let newX = x;
            let newY = y;

            if (x + rect.width > window.innerWidth) {
                newX = x - rect.width;
            }
            if (y + rect.height > window.innerHeight) {
                newY = y - rect.height;
            }
            setAdjustedPos({ x: newX, y: newY });
        }
    }, [x, y]);

    // Focus first item on mount
    useEffect(() => {
        itemRefs.current[0]?.focus();
    }, []);

    // Close on outside click
    useEffect(() => {
        const handleClick = () => onClose();
        const handleResize = () => onClose();
        const handleContextMenu = () => onClose();

        window.addEventListener('click', handleClick);
        window.addEventListener('resize', handleResize);
        window.addEventListener('contextmenu', handleContextMenu); // Close if right click elsewhere

        return () => {
            window.removeEventListener('click', handleClick);
            window.removeEventListener('resize', handleResize);
            window.removeEventListener('contextmenu', handleContextMenu);
        };
    }, [onClose]);

    // Keyboard navigation
    const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
        const enabledCount = menuItems.filter(m => m.enabled).length;
        const enabledItems = itemRefs.current;
        if (e.key === 'ArrowDown') {
            e.preventDefault();
            const activeIndex = enabledItems.findIndex(item => item === document.activeElement);
            const nextIndex = (activeIndex + 1) % enabledCount;
            enabledItems[nextIndex]?.focus();
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            const activeIndex = enabledItems.findIndex(item => item === document.activeElement);
            const nextIndex = (activeIndex - 1 + enabledCount) % enabledCount;
            enabledItems[nextIndex]?.focus();
        } else if (e.key === 'Escape') {
            e.preventDefault();
            onClose();
        } else if (e.key === 'Tab') {
            e.preventDefault(); // Trap focus
        }
    }, [menuItems, onClose]);

    let itemIndex = 0;

    return (
        <div
            ref={menuRef}
            role="menu"
            aria-orientation="vertical"
            aria-label={`Actions for ${file.name}`}
            onKeyDown={handleKeyDown}
            className="fixed z-50 min-w-[200px] bg-telegram-surface/95 backdrop-blur-xl border border-telegram-border rounded-lg shadow-2xl p-1.5 animate-in fade-in zoom-in-95 duration-100 flex flex-col gap-0.5"
            style={{ left: adjustedPos.x, top: adjustedPos.y }}
            onClick={(e) => e.stopPropagation()}
            onContextMenu={(e) => e.preventDefault()}
        >
            <div className="px-2 py-1.5 text-xs text-telegram-subtext font-medium truncate max-w-[180px] border-b border-telegram-border mb-1">
                {file.name}
            </div>

            {file.type !== 'folder' && (
                <button
                    role="menuitem"
                    ref={el => { if (previewEnabled && el) itemRefs.current[itemIndex++] = el; }}
                    onClick={() => previewEnabled && onPreview()}
                    disabled={!previewEnabled}
                    title={!previewEnabled ? (previewBlockedTitle || blockedTitle) : undefined}
                    className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-text hover:bg-telegram-hover rounded transition-colors text-left w-full disabled:opacity-40 disabled:cursor-not-allowed focus:outline-none focus:bg-telegram-hover"
                >
                    {isMediaFile(file.name) ? (
                        <>
                            <Play className="w-4 h-4 text-telegram-primary" />
                            Play
                        </>
                    ) : isPdfFile(file.name) ? (
                        <>
                            <FileText className="w-4 h-4 text-red-400" />
                            View PDF
                        </>
                    ) : (
                        <>
                            <Eye className="w-4 h-4 text-blue-500" />
                            Preview
                        </>
                    )}
                </button>
            )}

            {file.type === 'folder' && (
                <button
                    role="menuitem"
                    ref={el => { if (el) itemRefs.current[itemIndex++] = el; }}
                    onClick={onPreview}
                    className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-text hover:bg-telegram-hover rounded transition-colors text-left w-full focus:outline-none focus:bg-telegram-hover"
                >
                    <FolderOpen className="w-4 h-4 text-yellow-500" />
                    Open
                </button>
            )}

            <button
                role="menuitem"
                ref={el => { if (downloadEnabled && el) itemRefs.current[itemIndex++] = el; }}
                onClick={() => downloadEnabled && onDownload()}
                disabled={!downloadEnabled}
                title={!downloadEnabled ? (downloadBlockedTitle || blockedTitle) : undefined}
                className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-text hover:bg-telegram-hover rounded transition-colors text-left w-full disabled:opacity-40 disabled:cursor-not-allowed focus:outline-none focus:bg-telegram-hover"
            >
                <HardDrive className="w-4 h-4 text-green-500" />
                Download
            </button>

            {file.type !== 'folder' && onShare && (
                <button
                    role="menuitem"
                    ref={el => { if (shareEnabled && el) itemRefs.current[itemIndex++] = el; }}
                    onClick={() => shareEnabled && onShare()}
                    disabled={!shareEnabled}
                    title={!shareEnabled ? (shareBlockedTitle || blockedTitle) : undefined}
                    className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-text hover:bg-telegram-hover rounded transition-colors text-left w-full disabled:opacity-40 disabled:cursor-not-allowed focus:outline-none focus:bg-telegram-hover"
                >
                    <Link className="w-4 h-4 text-telegram-primary" />
                    Share Link
                </button>
            )}

            {file.type !== 'folder' && username && (
                <button
                    role="menuitem"
                    ref={el => { if (el) itemRefs.current[itemIndex++] = el; }}
                    onClick={async () => {
                        const url = `https://t.me/${username}/${file.id}`;
                        try {
                            await navigator.clipboard.writeText(url);
                            toast.success("Telegram 链接已复制");
                        } catch {
                            toast.error("复制链接失败");
                        }
                        onClose();
                    }}
                    className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-text hover:bg-telegram-hover rounded transition-colors text-left w-full focus:outline-none focus:bg-telegram-hover"
                >
                    <Copy className="w-4 h-4 text-telegram-primary" />
                    Copy Telegram Link
                </button>
            )}

            {file.type !== 'folder' && !username && (
                <button
                    role="menuitem"
                    disabled
                    title="Only available for public channels"
                    className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-subtext hover:bg-telegram-hover rounded transition-colors text-left w-full cursor-not-allowed opacity-50"
                >
                    <Copy className="w-4 h-4" />
                    Copy Telegram Link
                </button>
            )}

            <div className="h-px bg-telegram-border my-1" />

            <button
                role="menuitem"
                ref={el => { if (deleteEnabled && el) itemRefs.current[itemIndex++] = el; }}
                onClick={() => deleteEnabled && onDelete()}
                disabled={!deleteEnabled}
                title={!deleteEnabled ? blockedTitle : undefined}
                className="flex items-center gap-2 px-2 py-1.5 text-sm text-red-500 hover:bg-red-500/10 rounded transition-colors text-left w-full disabled:opacity-40 disabled:cursor-not-allowed focus:outline-none focus:bg-red-500/10"
            >
                <Trash2 className="w-4 h-4" />
                Delete
            </button>
        </div>
    );
}
