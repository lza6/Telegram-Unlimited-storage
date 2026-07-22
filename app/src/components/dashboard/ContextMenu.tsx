import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Eye, HardDrive, Trash2, FolderOpen, Pencil, Play, FileText, Link, Copy } from 'lucide-react';
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
}

export function ContextMenu({ x, y, file, onClose, onDownload, onDelete, onPreview, onShare, folders, activeFolderId }: ContextMenuProps) {
    const [adjustedPos, setAdjustedPos] = useState({ x, y });
    const menuRef = useRef<HTMLDivElement>(null);

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

    return (
        <div
            ref={menuRef}
            className="fixed z-50 min-w-[200px] bg-telegram-surface/95 backdrop-blur-xl border border-telegram-border rounded-lg shadow-2xl p-1.5 animate-in fade-in zoom-in-95 duration-100 flex flex-col gap-0.5"
            style={{ left: adjustedPos.x, top: adjustedPos.y }}
            onClick={(e) => e.stopPropagation()}
            onContextMenu={(e) => e.preventDefault()}
        >
            <div className="px-2 py-1.5 text-xs text-telegram-subtext font-medium truncate max-w-[180px] border-b border-telegram-border mb-1">
                {file.name}
            </div>

            {file.type !== 'folder' && (
                <button onClick={onPreview} className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-text hover:bg-telegram-hover rounded transition-colors text-left w-full">
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
                <button onClick={onPreview} className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-text hover:bg-telegram-hover rounded transition-colors text-left w-full">
                    <FolderOpen className="w-4 h-4 text-yellow-500" />
                    Open
                </button>
            )}

            <button onClick={onDownload} className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-text hover:bg-telegram-hover rounded transition-colors text-left w-full">
                <HardDrive className="w-4 h-4 text-green-500" />
                Download
            </button>

            {file.type !== 'folder' && onShare && (
                <button onClick={onShare} className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-text hover:bg-telegram-hover rounded transition-colors text-left w-full">
                    <Link className="w-4 h-4 text-telegram-primary" />
                    Share Link
                </button>
            )}

            {file.type !== 'folder' && (
                (() => {
                    const folder = folders?.find(f => f.id === file.folder_id) || folders?.find(f => f.id === activeFolderId);
                    const username = folder?.username || (folder as any)?.chat?.username || (folder as any)?.channel?.username;
                    
                    if (username) {
                        const handleCopyLink = async () => {
                            const url = `https://t.me/${username}/${file.id}`;
                            try {
                                await navigator.clipboard.writeText(url);
                                toast.success("Telegram link copied");
                            } catch (e) {
                                toast.error("Failed to copy link");
                            }
                            onClose();
                        };
                        return (
                            <button onClick={handleCopyLink} className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-text hover:bg-telegram-hover rounded transition-colors text-left w-full">
                                <Copy className="w-4 h-4 text-telegram-primary" />
                                Copy Telegram Link
                            </button>
                        );
                    } else {
                        return (
                            <button 
                                disabled 
                                title="Only available for public channels" 
                                className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-subtext hover:bg-telegram-hover rounded transition-colors text-left w-full cursor-not-allowed opacity-50"
                            >
                                <Copy className="w-4 h-4" />
                                Copy Telegram Link
                            </button>
                        );
                    }
                })()
            )}

            <button disabled className="flex items-center gap-2 px-2 py-1.5 text-sm text-telegram-subtext hover:bg-telegram-hover rounded transition-colors text-left w-full cursor-not-allowed opacity-50">
                <Pencil className="w-4 h-4" />
                Rename
            </button>

            <div className="h-px bg-telegram-border my-1" />

            <button onClick={onDelete} className="flex items-center gap-2 px-2 py-1.5 text-sm text-red-500 hover:bg-red-500/10 rounded transition-colors text-left w-full">
                <Trash2 className="w-4 h-4" />
                Delete
            </button>
        </div>
    );
}
