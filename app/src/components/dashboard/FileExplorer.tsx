import { useState, useMemo, useCallback, useRef, useEffect } from 'react';
import { Plus, ArrowUpDown, ArrowUp, ArrowDown, FolderUp } from 'lucide-react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { FileCard } from './FileCard';
import { EmptyState } from './EmptyState';
import { SkeletonGrid, SkeletonList } from './SkeletonLoader';
import { TelegramFile, TelegramFolder } from '../../types';
import { ContextMenu } from './ContextMenu';
import { FileListItem } from './FileListItem';

type SortField = 'name' | 'size' | 'date';
type SortDirection = 'asc' | 'desc';

interface FileExplorerProps {
    files: TelegramFile[];
    loading: boolean;
    error: Error | null;
    viewMode: 'grid' | 'list';
    selectedIds: number[];
    activeFolderId: number | null;
    onFileClick: (e: React.MouseEvent, id: number) => void;
    onDelete: (id: number) => void;
    onDownload: (id: number, name: string) => void;
    onPreview: (file: TelegramFile, orderedFiles?: TelegramFile[]) => void;
    onManualUpload: () => void;
    onFolderUpload: () => void;
    showFolderUpload: boolean;
    onSelectionClear: () => void;
    onToggleSelection: (id: number) => void;
    onDrop?: (e: React.DragEvent, folderId: number) => void;
    onDragStart?: (fileId: number) => void;
    onDragEnd?: () => void;
    onShare?: (file: TelegramFile) => void;
    folders?: TelegramFolder[];
    sessionOnline?: boolean;
    downloadReady?: boolean;
    previewReady?: boolean;
    shareReady?: boolean;
    deleteReady?: boolean;
    transferBlockedMessage?: string;
    downloadBlockedMessage?: string;
    previewBlockedMessage?: string;
    shareBlockedMessage?: string;
    isGlobalSearch?: boolean;
}


function useGridColumns(containerRef: React.RefObject<HTMLDivElement | null>) {
    const [columns, setColumns] = useState(4);
    const [containerWidth, setContainerWidth] = useState(800);

    useEffect(() => {
        if (!containerRef.current) return;

        const updateColumns = () => {
            const width = containerRef.current?.clientWidth || 800;
            setContainerWidth(width);
            if (width < 640) setColumns(2);
            else if (width < 768) setColumns(3);
            else if (width < 1024) setColumns(4);
            else if (width < 1280) setColumns(5);
            else setColumns(6);
        };

        updateColumns();
        const observer = new ResizeObserver(updateColumns);
        observer.observe(containerRef.current);
        return () => observer.disconnect();
    }, [containerRef]);

    return { columns, containerWidth };
}

export function FileExplorer({
    files, loading, error, viewMode, selectedIds, activeFolderId,
    onFileClick, onDelete, onDownload, onPreview, onManualUpload, onFolderUpload, showFolderUpload, onSelectionClear, onToggleSelection, onDrop, onDragStart, onDragEnd, onShare,
    folders, sessionOnline = true, downloadReady = sessionOnline, previewReady = downloadReady, shareReady = downloadReady, deleteReady = sessionOnline,
    transferBlockedMessage, downloadBlockedMessage, previewBlockedMessage, shareBlockedMessage, isGlobalSearch = false,
}: FileExplorerProps) {
    const blockedTitle = transferBlockedMessage || 'Session not ready';
    const downloadBlockedTitle = downloadBlockedMessage || blockedTitle;
    const previewBlockedTitle = previewBlockedMessage || downloadBlockedTitle;
    const shareBlockedTitle = shareBlockedMessage || downloadBlockedTitle;
    const guardUpload = (fn: () => void) => {
        if (!sessionOnline) return;
        fn();
    };

    const [sortField, setSortField] = useState<SortField>('name');
    const [sortDirection, setSortDirection] = useState<SortDirection>('asc');
    const [contextMenu, setContextMenu] = useState<{ x: number; y: number; file: TelegramFile } | null>(null);

    const parentRef = useRef<HTMLDivElement>(null);
    const { columns, containerWidth } = useGridColumns(parentRef);

    const GAP = 6;
    const cardWidth = (containerWidth - (GAP * (columns - 1))) / columns;
    const cardHeight = cardWidth * 0.75; // aspect-[4/3]
    const rowHeight = Math.max(cardHeight + GAP, 150);

    const handleContextMenu = useCallback((e: React.MouseEvent, file: TelegramFile) => {
        e.preventDefault();
        e.stopPropagation();
        setContextMenu({ x: e.clientX, y: e.clientY, file });
    }, []);

    const sortedFiles = useMemo(() => {
        return [...files].sort((a, b) => {
            let comparison = 0;
            switch (sortField) {
                case 'name':
                    comparison = a.name.localeCompare(b.name);
                    break;
                case 'size':
                    comparison = (a.size || 0) - (b.size || 0);
                    break;
                case 'date':
                    comparison = (a.created_at || '').localeCompare(b.created_at || '');
                    break;
            }
            return sortDirection === 'asc' ? comparison : -comparison;
        });
    }, [files, sortField, sortDirection]);

    const handlePreviewRequest = useCallback((file: TelegramFile) => {
        if (file.type === 'folder') {
            onFileClick({ preventDefault: () => { }, stopPropagation: () => { } } as React.MouseEvent, file.id);
            return;
        }
        onPreview(file, sortedFiles);
    }, [onPreview, onFileClick, sortedFiles]);


    const gridRows = useMemo(() => {
        const rows: (TelegramFile | 'upload' | 'upload-folder')[][] = [];
        const tail: ('upload' | 'upload-folder')[] = ['upload'];
        if (showFolderUpload) tail.push('upload-folder');
        const itemsWithUpload: (TelegramFile | 'upload' | 'upload-folder')[] = [...sortedFiles, ...tail];
        for (let i = 0; i < itemsWithUpload.length; i += columns) {
            rows.push(itemsWithUpload.slice(i, i + columns));
        }
        return rows;
    }, [sortedFiles, columns, showFolderUpload]);


    const listItems = useMemo(() => {
        const tail: ('upload' | 'upload-folder')[] = ['upload'];
        if (showFolderUpload) tail.push('upload-folder');
        return [...sortedFiles, ...tail];
    }, [sortedFiles, activeFolderId, showFolderUpload]);


    const gridVirtualizer = useVirtualizer({
        count: gridRows.length,
        getScrollElement: () => parentRef.current,
        estimateSize: useCallback(() => rowHeight, [rowHeight]),
        overscan: 2,
        gap: GAP,
    });


    useEffect(() => {
        gridVirtualizer.measure();
    }, [rowHeight, gridVirtualizer]);

    const listVirtualizer = useVirtualizer({
        count: listItems.length,
        getScrollElement: () => parentRef.current,
        estimateSize: () => 48,
        overscan: 5,
    });

    const handleSort = (field: SortField) => {
        if (sortField === field) {
            setSortDirection(d => d === 'asc' ? 'desc' : 'asc');
        } else {
            setSortField(field);
            setSortDirection('asc');
        }
    };

    const SortIcon = ({ field }: { field: SortField }) => {
        if (sortField !== field) return <ArrowUpDown className="w-3 h-3 opacity-30" />;
        return sortDirection === 'asc'
            ? <ArrowUp className="w-3 h-3 text-telegram-primary" />
            : <ArrowDown className="w-3 h-3 text-telegram-primary" />;
    };

    if (loading) {
        return (
            <div className="flex-1 p-6 overflow-auto">
                {viewMode === 'grid' ? <SkeletonGrid /> : <SkeletonList />}
            </div>
        );
    }

    if (error) {
        const message = error instanceof Error ? error.message : String(error);
        return (
            <div className="flex-1 p-6 flex flex-col justify-center items-center text-red-400 gap-2">
                <span>Error loading files</span>
                <span className="text-xs text-red-400/80 max-w-md text-center break-words">{message}</span>
            </div>
        );
    }

    if (files.length === 0) {
        if (isGlobalSearch && !loading) {
            return (
                <div className="flex-1 p-6 overflow-auto">
                    <EmptyState onUpload={() => guardUpload(onManualUpload)} uploadEnabled={sessionOnline} blockedTitle={blockedTitle} variant="search" />
                </div>
            );
        }
        return (
            <div className="flex-1 p-6 overflow-auto">
                <EmptyState onUpload={() => guardUpload(onManualUpload)} uploadEnabled={sessionOnline} blockedTitle={blockedTitle} />
            </div>
        );
    }

    return (
        <div
            ref={parentRef}
            className="flex-1 p-6 overflow-auto custom-scrollbar"
            onClick={(e) => {
                if (e.target === e.currentTarget) onSelectionClear();
            }}
        >
            {viewMode === 'grid' ? (
                <>

                    <div className="flex items-center gap-2 mb-4 text-xs text-telegram-subtext">
                        <span>Sort by:</span>
                        <button
                            onClick={() => handleSort('name')}
                            className={`px-2 py-1 rounded flex items-center gap-1 hover:bg-white/5 ${sortField === 'name' ? 'text-telegram-primary' : ''}`}
                        >
                            Name <SortIcon field="name" />
                        </button>
                        <button
                            onClick={() => handleSort('size')}
                            className={`px-2 py-1 rounded flex items-center gap-1 hover:bg-white/5 ${sortField === 'size' ? 'text-telegram-primary' : ''}`}
                        >
                            Size <SortIcon field="size" />
                        </button>
                        <button
                            onClick={() => handleSort('date')}
                            className={`px-2 py-1 rounded flex items-center gap-1 hover:bg-white/5 ${sortField === 'date' ? 'text-telegram-primary' : ''}`}
                        >
                            Date <SortIcon field="date" />
                        </button>
                    </div>


                    <div
                        className="relative w-full"
                        style={{ height: `${gridVirtualizer.getTotalSize()}px` }}
                    >
                        {gridVirtualizer.getVirtualItems().map((virtualRow) => {
                            const row = gridRows[virtualRow.index];
                            return (
                                <div
                                    key={virtualRow.key}
                                    className="absolute top-0 left-0 w-full grid"
                                    style={{
                                        height: `${cardHeight}px`,
                                        transform: `translateY(${virtualRow.start}px)`,
                                        gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
                                        gap: `${GAP}px`,
                                    }}
                                >
                                    {row.map((item) => {
                                        if (item === 'upload') {
                                            return (
                                                <button
                                                    key="upload"
                                                    onClick={(e) => { e.stopPropagation(); guardUpload(onManualUpload); }}
                                                    disabled={!sessionOnline}
                                                    title={!sessionOnline ? blockedTitle : undefined}
                                                    className="border-2 border-dashed border-telegram-border rounded-xl flex flex-col items-center justify-center text-telegram-subtext hover:border-telegram-primary hover:text-telegram-primary transition-all group disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:border-telegram-border disabled:hover:text-telegram-subtext"
                                                    style={{ height: `${cardHeight}px` }}
                                                >
                                                    <Plus className="w-8 h-8 mb-2 group-hover:scale-110 transition-transform" />
                                                    <span className="text-sm font-medium">Upload File</span>
                                                </button>
                                            );
                                        }
                                        if (item === 'upload-folder') {
                                            return (
                                                <button
                                                    key="upload-folder"
                                                    onClick={(e) => { e.stopPropagation(); guardUpload(onFolderUpload); }}
                                                    disabled={!sessionOnline}
                                                    title={!sessionOnline ? blockedTitle : undefined}
                                                    className="border-2 border-dashed border-telegram-border rounded-xl flex flex-col items-center justify-center text-telegram-subtext hover:border-telegram-primary hover:text-telegram-primary transition-all group disabled:opacity-40 disabled:cursor-not-allowed disabled:hover:border-telegram-border disabled:hover:text-telegram-subtext"
                                                    style={{ height: `${cardHeight}px` }}
                                                >
                                                    <FolderUp className="w-8 h-8 mb-2 group-hover:scale-110 transition-transform" />
                                                    <span className="text-sm font-medium">Upload Folder</span>
                                                </button>
                                            );
                                        }
                                        const file = item;
                                        return (
                                            <FileCard
                                                key={file.id}
                                                file={file}
                                                isSelected={selectedIds.includes(file.id)}
                                                onClick={(e) => onFileClick(e, file.id)}
                                                onContextMenu={(e) => handleContextMenu(e, file)}
                                                onDelete={() => onDelete(file.id)}
                                                onDownload={() => onDownload(file.id, file.name)}
                                                onPreview={() => handlePreviewRequest(file)}
                                                onShare={onShare && file.type !== 'folder' ? () => onShare(file) : undefined}
                                                onDrop={onDrop}
                                                onDragStart={onDragStart}
                                                onDragEnd={onDragEnd}
                                                activeFolderId={activeFolderId}
                                                height={cardHeight}
                                                onToggleSelection={() => onToggleSelection(file.id)}
                                                transferEnabled={sessionOnline}
                                                previewEnabled={previewReady}
                                                downloadEnabled={downloadReady}
                                                shareEnabled={shareReady}
                                                deleteEnabled={deleteReady}
                                                blockedTitle={blockedTitle}
                                                downloadBlockedTitle={downloadBlockedTitle}
                                                previewBlockedTitle={previewBlockedTitle}
                                                shareBlockedTitle={shareBlockedTitle}
                                            />
                                        );
                                    })}
                                </div>
                            );
                        })}
                    </div>
                </>
            ) : (
                <div className="flex flex-col w-full">
                    {/* List Header */}
                    <div className="grid grid-cols-[2rem_2fr_6rem_8rem] gap-4 px-4 py-2 text-xs font-semibold text-telegram-subtext border-b border-telegram-border mb-2 select-none items-center">
                        <div className="text-center">#</div>
                        <button onClick={() => handleSort('name')} className="flex items-center gap-1 hover:text-telegram-text transition-colors">
                            Name <SortIcon field="name" />
                        </button>
                        <button onClick={() => handleSort('size')} className="flex items-center gap-1 justify-end hover:text-telegram-text transition-colors">
                            Size <SortIcon field="size" />
                        </button>
                        <button onClick={() => handleSort('date')} className="flex items-center gap-1 justify-end hover:text-telegram-text transition-colors">
                            Date <SortIcon field="date" />
                        </button>
                    </div>


                    <div
                        className="relative w-full"
                        style={{ height: `${listVirtualizer.getTotalSize()}px` }}
                    >
                        {listVirtualizer.getVirtualItems().map((virtualItem) => {
                            const item = listItems[virtualItem.index];
                            if (item === 'upload') {
                                return (
                                    <div
                                        key="upload"
                                        className="absolute top-0 left-0 w-full"
                                        style={{ transform: `translateY(${virtualItem.start}px)` }}
                                    >
                                        <button
                                            onClick={(e) => { e.stopPropagation(); guardUpload(onManualUpload); }}
                                            disabled={!sessionOnline}
                                            title={!sessionOnline ? blockedTitle : undefined}
                                            className="flex items-center gap-4 px-4 py-3 rounded-lg cursor-pointer border border-dashed border-telegram-border text-telegram-subtext hover:text-telegram-text hover:bg-telegram-hover w-full disabled:opacity-40 disabled:cursor-not-allowed"
                                        >
                                            <div className="w-5 h-5 flex items-center justify-center"><Plus className="w-4 h-4" /></div>
                                            <span className="text-sm font-medium">Upload File...</span>
                                        </button>
                                    </div>
                                );
                            }
                            if (item === 'upload-folder') {
                                return (
                                    <div
                                        key="upload-folder"
                                        className="absolute top-0 left-0 w-full"
                                        style={{ transform: `translateY(${virtualItem.start}px)` }}
                                    >
                                        <button
                                            onClick={(e) => { e.stopPropagation(); guardUpload(onFolderUpload); }}
                                            disabled={!sessionOnline}
                                            title={!sessionOnline ? blockedTitle : undefined}
                                            className="flex items-center gap-4 px-4 py-3 rounded-lg cursor-pointer border border-dashed border-telegram-border text-telegram-subtext hover:text-telegram-text hover:bg-telegram-hover w-full disabled:opacity-40 disabled:cursor-not-allowed"
                                        >
                                            <div className="w-5 h-5 flex items-center justify-center"><FolderUp className="w-4 h-4" /></div>
                                            <span className="text-sm font-medium">Upload Folder...</span>
                                        </button>
                                    </div>
                                );
                            }
                            const file = item;
                            return (
                                <div
                                    key={file.id}
                                    className="absolute top-0 left-0 w-full"
                                    style={{ transform: `translateY(${virtualItem.start}px)` }}
                                >
                                    <FileListItem
                                        file={file}
                                        selectedIds={selectedIds}
                                        onFileClick={onFileClick}
                                        handleContextMenu={handleContextMenu}
                                        onDragStart={onDragStart}
                                        onDragEnd={onDragEnd}
                                        onDrop={onDrop}
                                        onPreview={handlePreviewRequest}
                                        onDownload={onDownload}
                                        onDelete={onDelete}
                                        onShare={onShare}
                                        transferEnabled={sessionOnline}
                                        previewEnabled={previewReady}
                                        downloadEnabled={downloadReady}
                                        shareEnabled={shareReady}
                                        deleteEnabled={deleteReady}
                                        blockedTitle={blockedTitle}
                                        downloadBlockedTitle={downloadBlockedTitle}
                                        previewBlockedTitle={previewBlockedTitle}
                                        shareBlockedTitle={shareBlockedTitle}
                                    />
                                </div>
                            );
                        })}
                    </div>
                </div>
            )}

            {contextMenu && (
                <ContextMenu
                    x={contextMenu.x}
                    y={contextMenu.y}
                    file={contextMenu.file}
                    onClose={() => setContextMenu(null)}
                    onDownload={() => {
                        onDownload(contextMenu.file.id, contextMenu.file.name);
                        setContextMenu(null);
                    }}
                    onDelete={() => {
                        onDelete(contextMenu.file.id);
                        setContextMenu(null);
                    }}
                    onPreview={() => {
                        if (contextMenu.file.type === 'folder') {
                            onFileClick({ preventDefault: () => { }, stopPropagation: () => { } } as React.MouseEvent, contextMenu.file.id);
                        } else {
                            handlePreviewRequest(contextMenu.file);
                        }
                        setContextMenu(null);
                    }}
                    onShare={onShare ? () => {
                        onShare(contextMenu.file);
                        setContextMenu(null);
                    } : undefined}
                    folders={folders}
                    activeFolderId={activeFolderId}
                    transferEnabled={sessionOnline}
                    previewEnabled={previewReady}
                    downloadEnabled={downloadReady}
                    shareEnabled={shareReady}
                    deleteEnabled={deleteReady}
                    blockedTitle={blockedTitle}
                    downloadBlockedTitle={downloadBlockedTitle}
                    previewBlockedTitle={previewBlockedTitle}
                    shareBlockedTitle={shareBlockedTitle}
                />
            )}
        </div>
    )
}
