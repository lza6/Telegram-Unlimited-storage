import { motion } from 'framer-motion';
import { useState, useEffect, memo, useRef } from 'react';
import { Folder, Eye, Trash2, Share2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { TelegramFile } from '../../types';
import { FileTypeIcon } from '../FileTypeIcon';

interface FileCardProps {
    file: TelegramFile;
    onDelete: () => void;
    onDownload: () => void;
    onPreview?: () => void;
    onShare?: () => void;
    isSelected: boolean;
    onClick?: (e: React.MouseEvent) => void;
    onContextMenu?: (e: React.MouseEvent) => void;
    onDrop?: (e: React.DragEvent, folderId: number) => void;
    onDragStart?: (fileId: number) => void;
    onDragEnd?: () => void;
    activeFolderId?: number | null;
    height?: number;
    onToggleSelection?: () => void;
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

// Check if file is an image type that can have a thumbnail
function isImageFile(filename: string): boolean {
    const ext = filename.split('.').pop()?.toLowerCase() || '';
    return ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp'].includes(ext);
}

export const FileCard = memo(function FileCard({
    file, onDelete, onDownload, onPreview, onShare, isSelected, onClick, onContextMenu, onDrop, onDragStart, onDragEnd,
    activeFolderId, height, onToggleSelection,
    transferEnabled = true,
    previewEnabled = transferEnabled,
    downloadEnabled = transferEnabled,
    shareEnabled = transferEnabled,
    deleteEnabled = transferEnabled,
    blockedTitle,
    downloadBlockedTitle,
    previewBlockedTitle,
    shareBlockedTitle,
}: FileCardProps) {
    const isFolder = file.type === 'folder';
    const peerFolderId = file.folder_id ?? activeFolderId ?? null;
    const [isDragOver, setIsDragOver] = useState(false);
    const [thumbnail, setThumbnail] = useState<string | null>(null);
    const [thumbnailLoading, setThumbnailLoading] = useState(false);
    const [isVisible, setIsVisible] = useState(false);
    const cardRef = useRef<HTMLDivElement>(null);

    // Swipe-to-delete state
    const [swipeOffset, setSwipeOffset] = useState(0);
    const [isSwiping, setIsSwiping] = useState(false);
    const touchStartX = useRef(0);
    const touchStartY = useRef(0);
    const SWIPE_THRESHOLD = 80;
    const MAX_SWIPE = 120;

    // Lazy load thumbnail for image files (with intersection observer)
    useEffect(() => {
        if (isFolder || !isImageFile(file.name) || !previewEnabled) return;
        if (!cardRef.current) return;

        const observer = new IntersectionObserver(
            (entries) => {
                if (entries[0].isIntersecting) {
                    setIsVisible(true);
                }
            },
            { rootMargin: '100px' }
        );

        observer.observe(cardRef.current);
        return () => observer.disconnect();
    }, [isFolder, file.name, previewEnabled]);

    // Load thumbnail when visible
    useEffect(() => {
        if (!isVisible || thumbnail) return;

        let cancelled = false;
        setThumbnailLoading(true);

        invoke<string>('cmd_get_thumbnail', {
            messageId: file.id,
            folderId: peerFolderId
        }).then((result) => {
            if (!cancelled && result) {
                setThumbnail(result);
            }
        }).catch(() => {
            // Silently fail - will show icon instead
        }).finally(() => {
            if (!cancelled) setThumbnailLoading(false);
        });

        return () => { cancelled = true; };
    }, [isVisible, thumbnail, file.id, peerFolderId]);

    const guardPreview = (action: () => void) => {
        if (!previewEnabled) return;
        action();
    };

    // Touch handlers for swipe-to-delete (mobile only)
    const handleTouchStart = (e: React.TouchEvent) => {
        touchStartX.current = e.touches[0].clientX;
        touchStartY.current = e.touches[0].clientY;
        setIsSwiping(true);
    };

    const handleTouchMove = (e: React.TouchEvent) => {
        if (!isSwiping) return;
        const deltaX = e.touches[0].clientX - touchStartX.current;
        const deltaY = e.touches[0].clientY - touchStartY.current;

        // Only allow horizontal swipe
        if (Math.abs(deltaY) > Math.abs(deltaX) * 2) {
            setIsSwiping(false);
            setSwipeOffset(0);
            return;
        }

        // Only allow left swipe for delete
        if (deltaX < 0 && deleteEnabled) {
            const offset = Math.max(deltaX, -MAX_SWIPE);
            setSwipeOffset(offset);
        }
    };

    const handleTouchEnd = () => {
        setIsSwiping(false);
        if (swipeOffset < -SWIPE_THRESHOLD && deleteEnabled) {
            onDelete();
        }
        setSwipeOffset(0);
    };

    return (
        <div
            className="relative overflow-hidden"
            onContextMenu={onContextMenu}
            onClick={onClick}
            onTouchStart={handleTouchStart}
            onTouchMove={handleTouchMove}
            onTouchEnd={handleTouchEnd}
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
        >
            {/* Swipe delete indicator background */}
            {swipeOffset < -10 && (
                <div
                    className="absolute inset-y-0 right-0 flex items-center justify-center bg-red-500 text-white w-24 transition-opacity"
                    style={{ opacity: Math.min(1, Math.abs(swipeOffset) / SWIPE_THRESHOLD) }}
                >
                    <Trash2 className="w-6 h-6" />
                </div>
            )}
            <motion.div
                ref={cardRef}
                layout
                draggable={transferEnabled && !isFolder}
                role="button"
                aria-label={`${isFolder ? 'Folder' : 'File'}: ${file.name}`}
                tabIndex={0}
                onKeyDown={(e: any) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                        e.preventDefault();
                        if (onClick) onClick(e);
                    }
                }}
                onDragStart={(e: any) => {
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
                whileHover={{ y: -4 }}
                className={`group cursor-pointer bg-telegram-surface rounded-xl overflow-hidden border hover:shadow-[0_4px_20px_rgba(0,0,0,0.2)] transition-all relative
                ${isSelected ? 'border-telegram-primary bg-telegram-primary/5 ring-1 ring-telegram-primary' : 'border-telegram-border hover:border-telegram-primary/50'}
                ${isDragOver ? 'ring-2 ring-telegram-primary bg-telegram-primary/20 scale-105' : ''}`}
                style={{
                    ...(height ? { height: `${height}px` } : { aspectRatio: '4/3' }),
                    transform: `translateX(${swipeOffset}px)`,
                    transition: isSwiping ? 'none' : 'transform 0.2s ease-out',
                }}
            >
                {/* Thumbnail or Icon */}
                {thumbnail ? (
                    <div className="absolute inset-0">
                        <img
                            src={thumbnail}
                            alt={file.name}
                            className="w-full h-full object-cover"
                        />
                        {/* Gradient overlay for text readability */}
                        <div className="absolute inset-0 bg-gradient-to-t from-black/70 via-transparent to-transparent" />
                    </div>
                ) : (
                    <div className="absolute inset-0 flex items-center justify-center p-4">
                        {isFolder ? (
                            <Folder className="w-12 h-12 text-telegram-primary" />
                        ) : thumbnailLoading && isImageFile(file.name) ? (
                            <div className="w-8 h-8 border-2 border-telegram-primary/30 border-t-telegram-primary rounded-full animate-spin" />
                        ) : (
                            <FileTypeIcon filename={file.name} size="lg" />
                        )}
                    </div>
                )}

                {/* Selection Checkmark */}
                <div
                    role="checkbox"
                    aria-checked={isSelected}
                    aria-label={`Select ${file.name}`}
                    tabIndex={0}
                    onClick={(e) => {
                        e.stopPropagation();
                        if (onToggleSelection) onToggleSelection();
                    }}
                    onKeyDown={(e) => {
                        if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault();
                            if (onToggleSelection) onToggleSelection();
                        }
                    }}
                    className={`absolute top-2 left-2 w-5 h-5 rounded-full border flex items-center justify-center transition-all z-10 cursor-pointer ${isSelected ? 'bg-telegram-primary border-telegram-primary' : 'border-white/50 bg-black/30 opacity-0 group-hover:opacity-100'}`}
                >
                    {isSelected && <div className="w-1.5 h-1.5 bg-black rounded-full" />}
                </div>

                {/* File info overlay at bottom */}
                <div className={`absolute bottom-0 left-0 right-0 p-3 ${thumbnail ? 'text-white' : 'text-telegram-text'}`}>
                    <h3 className="text-sm font-medium truncate w-full" title={file.name}>{file.name}</h3>
                    <p className={`text-xs mt-0.5 ${thumbnail ? 'text-white/70' : 'text-telegram-subtext'}`}>{file.sizeStr}</p>
                </div>

                {/* Quick actions on hover */}
                <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity flex gap-1 z-10">
                    {!isFolder && onShare && (
                        <button onClick={(e) => { e.stopPropagation(); if (shareEnabled) onShare(); }} disabled={!shareEnabled} title={!shareEnabled ? (shareBlockedTitle || blockedTitle) : 'Share'} className="file-action-btn p-1 bg-black/50 rounded-full hover:bg-blue-500 hover:text-white text-white/70 disabled:opacity-40 disabled:cursor-not-allowed" aria-label={`Share ${file.name}`}>
                            <Share2 className="w-3 h-3" />
                        </button>
                    )}
                    <button onClick={(e) => { e.stopPropagation(); guardPreview(() => { if (onPreview) onPreview(); }); }} disabled={!previewEnabled} title={!previewEnabled ? (previewBlockedTitle || blockedTitle) : (isFolder ? 'Open' : 'Preview')} className="file-action-btn p-1 bg-black/50 rounded-full hover:bg-telegram-primary hover:text-white text-white/70 disabled:opacity-40 disabled:cursor-not-allowed" aria-label={isFolder ? `Open ${file.name}` : `Preview ${file.name}`}>
                        <Eye className="w-3 h-3" />
                    </button>
                    <button onClick={(e) => { e.stopPropagation(); if (downloadEnabled) onDownload(); }} disabled={!downloadEnabled} title={!downloadEnabled ? (downloadBlockedTitle || blockedTitle) : 'Download'} className="file-action-btn p-1 bg-black/50 rounded-full hover:bg-green-500 hover:text-white text-white/70 disabled:opacity-40 disabled:cursor-not-allowed" aria-label={`Download ${file.name}`}>
                        <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="w-3 h-3"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
                    </button>
                    <button onClick={(e) => { e.stopPropagation(); if (deleteEnabled) onDelete(); }} disabled={!deleteEnabled} title={!deleteEnabled ? blockedTitle : 'Delete'} className="file-action-btn p-1 bg-black/50 rounded-full hover:bg-red-500 hover:text-white text-white/70 disabled:opacity-40 disabled:cursor-not-allowed" aria-label={`Delete ${file.name}`}>
                        <Trash2 className="w-3 h-3" />
                    </button>
                </div>
            </motion.div>
        </div>
    );
});
