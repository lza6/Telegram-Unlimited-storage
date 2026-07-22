import { useState, useEffect, useCallback, useRef } from 'react';
import { AnimatePresence } from 'framer-motion';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { Menu } from 'lucide-react';

import {
    formatBytes, isMediaFile, isPdfFile, planMoveGroups, resolveFileFolderId,
    fileBelongsToFolder, filterFilesExcludingIds,
    remapMovedFilesInList, remapOpenFileAfterMove,
    pruneSelectedIdsAfterDelete,
} from '../utils';
import { executeMoveGroups } from '../lib/moveExecution';
import { MoveFilesPayload, TelegramFile, BandwidthStats } from '../types';

// Components
import { Sidebar } from './dashboard/Sidebar';
import { TopBar } from './dashboard/TopBar';
import { FileExplorer } from './dashboard/FileExplorer';
import { UploadQueue } from './dashboard/UploadQueue';
import { DownloadQueue } from './dashboard/DownloadQueue';
import { MoveToFolderModal } from './dashboard/MoveToFolderModal';
import { PreviewModal } from './dashboard/PreviewModal';
import { MediaPlayer } from './dashboard/MediaPlayer';
import { ExternalDropBlocker } from './dashboard/ExternalDropBlocker';
import { PdfViewer } from './dashboard/PdfViewer';
import { SettingsModal } from './dashboard/SettingsModal';
import { ShareDialog } from './dashboard/ShareDialog';

// Hooks
import { useTelegramConnection } from '../hooks/useTelegramConnection';
import { useFileOperations } from '../hooks/useFileOperations';
import { useFileUpload } from '../hooks/useFileUpload';
import { useFileDownload } from '../hooks/useFileDownload';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';
import { useSettings } from '../context/SettingsContext';
import {
    canDownloadFiles,
    canPreviewFiles,
    canShareFiles,
    canTransferFiles,
    connectionStatusLabel,
    isBotIndexReady,
    isServiceReady,
} from '../types/connection';
import { isSessionLostError } from '../utils/sessionError';
import {
    buildRebuildFolderIds,
    formatIndexRebuildBackgroundFailureMessage,
    isGlobalSearchActive,
    shouldRebuildIndexBeforeGlobalSearch,
} from '../lib/searchPure';
import {
    bulkMoveBlockedMessage,
    canBulkMoveInTransportMode,
} from '../lib/filesPure';

export function Dashboard({ onLogout }: { onLogout: () => void }) {
    const queryClient = useQueryClient();


    const {
        store, folders, activeFolderId, setActiveFolderId, isSyncing, isConnected,
        handleLogout, handleSyncFolders, handleCreateFolder, handleFolderDelete,
        forceLogout, connectionStatus,
    } = useTelegramConnection(onLogout);

    const onSessionError = useCallback((_msg: string) => {
        forceLogout();
    }, [forceLogout]);


    const { settings, updateSetting } = useSettings();
    const viewMode = settings.viewMode;
    const setViewMode = (mode: 'grid' | 'list') => updateSetting('viewMode', mode);

    const [previewFile, setPreviewFile] = useState<TelegramFile | null>(null);
    const [selectedIds, setSelectedIds] = useState<number[]>([]);
    const [showMoveModal, setShowMoveModal] = useState(false);
    const [showSettings, setShowSettings] = useState(false);
    const [sidebarCollapsed, setSidebarCollapsed] = useState(true); // Mobile: collapsed by default
    const [searchTerm, setSearchTerm] = useState("");
    const [searchResults, setSearchResults] = useState<TelegramFile[]>([]);
    const [isSearching, setIsSearching] = useState(false);
    const [internalDragFileId, setInternalDragFileId] = useState<number | null>(null);
    const internalDragRef = useRef<number | null>(null);
    // Sync ref on every change so handlers always read latest
    internalDragRef.current = internalDragFileId;
    const globalSearchActiveRef = useRef(false);

    const [playingFile, setPlayingFile] = useState<TelegramFile | null>(null);
    const [pdfFile, setPdfFile] = useState<TelegramFile | null>(null);
    const [shareFile, setShareFile] = useState<TelegramFile | null>(null);
    const [previewContextFiles, setPreviewContextFiles] = useState<TelegramFile[]>([]);
    const [previewContextIndex, setPreviewContextIndex] = useState(-1);

    const closePreviewState = useCallback(() => {
        setPreviewFile(null);
        setPlayingFile(null);
        setPdfFile(null);
    }, []);

    const closePreviewIfRemoved = useCallback((removedIds: number[]) => {
        const openId = previewFile?.id ?? playingFile?.id ?? pdfFile?.id ?? shareFile?.id;
        if (openId && removedIds.includes(openId)) {
            closePreviewState();
            setShareFile(null);
        }
    }, [previewFile, playingFile, pdfFile, shareFile, closePreviewState]);

    const handleFilesRemoved = useCallback((removedIds: number[]) => {
        closePreviewIfRemoved(removedIds);
        setSearchResults((prev) => filterFilesExcludingIds(prev, removedIds));
        setPreviewContextFiles((prev) => filterFilesExcludingIds(prev, removedIds));
    }, [closePreviewIfRemoved]);

    const handleFilesMoved = useCallback((payload: MoveFilesPayload) => {
        setSearchResults((prev) => remapMovedFilesInList(prev, payload));
        setPreviewContextFiles((prev) => remapMovedFilesInList(prev, payload));
        setPreviewFile((prev) => remapOpenFileAfterMove(prev, payload));
        setPlayingFile((prev) => remapOpenFileAfterMove(prev, payload));
        setPdfFile((prev) => remapOpenFileAfterMove(prev, payload));
        setShareFile((prev) => remapOpenFileAfterMove(prev, payload));
        if (searchTerm.length > 2 && payload.oldIds.length > 0) {
            toast.success('文件已移动 — 搜索结果已更新');
        }
    }, [searchTerm]);

    const transferReady = canTransferFiles(connectionStatus);
    const transferBlockedMessage = connectionStatusLabel(connectionStatus);

    const { data: apiHealth } = useQuery({
        queryKey: ['api-health'],
        queryFn: () =>
            invoke<{ transport_mode: string; ready: boolean }>('cmd_get_api_health'),
        refetchInterval: 30_000,
        enabled: !!store,
        retry: false,
    });

    const serviceReady = isServiceReady({
        connectionStatus,
        apiHealthReady: apiHealth?.ready,
    });
    const botIndexReady = isBotIndexReady({
        apiHealthReady: apiHealth?.ready,
        transportMode: apiHealth?.transport_mode,
    });
    const downloadReady = canDownloadFiles({ transferReady, botIndexReady });
    const previewReady = canPreviewFiles({ transferReady, botIndexReady });
    const shareReady = canShareFiles({ transferReady, botIndexReady });
    const deleteReady = transferReady || botIndexReady;
    const sessionOnline = transferReady;
    const downloadBlockedMessage = downloadReady
        ? undefined
        : (transferBlockedMessage || 'Bot 模式未就绪 — 请启用本地 API 并确认传输模式为 Bot');
    const previewBlockedMessage = previewReady
        ? undefined
        : (downloadBlockedMessage || transferBlockedMessage);
    const shareBlockedMessage = shareReady
        ? undefined
        : (downloadBlockedMessage || transferBlockedMessage);

    const { data: allFiles = [], isLoading, error } = useQuery({
        queryKey: ['files', activeFolderId],
        queryFn: () => invoke<any[]>('cmd_get_files', { folderId: activeFolderId }).then(res => res.map(f => ({
            ...f,
            sizeStr: formatBytes(f.size),
            type: f.icon_type || (f.name.endsWith('/') ? 'folder' : 'file')
        }))),
        enabled: !!store && serviceReady,
    });

    const displayedFiles = searchTerm.length > 2
        ? searchResults
        : allFiles.filter((f: TelegramFile) => f.name.toLowerCase().includes(searchTerm.toLowerCase()));

    const { data: bandwidth } = useQuery({
        queryKey: ['bandwidth'],
        queryFn: () => invoke<BandwidthStats>('cmd_get_bandwidth'),
        refetchInterval: 5000,
        enabled: !!store && transferReady,
    });

    const transportMode = apiHealth?.transport_mode;
    const bulkMoveAllowed =
        transportMode == null || canBulkMoveInTransportMode(transportMode);

    const transferOpts = {
        canTransfer: () => transferReady,
        transferBlockedMessage,
        canDownload: () => downloadReady,
        downloadBlockedMessage,
        canIndexDelete: () => botIndexReady,
        indexDeleteBlockedMessage: 'Bot 模式未就绪 — 请启用本地 API 并确认传输模式为 Bot',
        canBulkMove: () => bulkMoveAllowed,
        bulkMoveBlockedMessage: bulkMoveBlockedMessage(transportMode, 'desktop'),
    };

    const { downloadQueue, queueDownload, queueBulkDownload, clearFinished: clearDownloads, cancelAll: cancelDownloads, cancelItem: cancelDownloadItem, retryItem: retryDownloadItem } = useFileDownload(store, { onSessionError, ...transferOpts });

    const {
        handleDelete, handleBulkDelete, handleBulkDownload,
        handleBulkMove, handleDownloadFolder,
    } = useFileOperations(activeFolderId, selectedIds, setSelectedIds, displayedFiles, queueBulkDownload, onSessionError, {
        ...transferOpts,
        onFilesRemoved: handleFilesRemoved,
        onFilesMoved: handleFilesMoved,
    });

    const { uploadQueue, enqueueUploadPaths, handleManualUpload, handleFolderUpload, cancelAll: cancelUploads, cancelItem: cancelUploadItem, retryItem: retryUploadItem, clearFinished: clearUploads } = useFileUpload(activeFolderId, store, { onSessionError, ...transferOpts });


    const handleSelectAll = useCallback(() => {
        setSelectedIds(displayedFiles.map(f => f.id));
    }, [displayedFiles]);

    const handleKeyboardDelete = useCallback(() => {
        if (selectedIds.length > 0) {
            handleBulkDelete();
        }
    }, [selectedIds, handleBulkDelete]);

    const handleEscape = useCallback(() => {
        if (shareFile) {
            setShareFile(null);
            return;
        }
        if (showSettings) {
            setShowSettings(false);
            return;
        }
        if (showMoveModal) {
            setShowMoveModal(false);
            return;
        }
        setSelectedIds([]);
        setSearchTerm("");
        setPreviewFile(null);
        setPlayingFile(null);
        setPdfFile(null);
    }, [shareFile, showSettings, showMoveModal]);

    const handleFocusSearch = useCallback(() => {
        const searchInput = document.querySelector('input[placeholder="Search files..."]') as HTMLInputElement;
        if (searchInput) {
            searchInput.focus();
            searchInput.select();
        }
    }, []);

    const handleTransportSwitched = useCallback(() => {
        queryClient.invalidateQueries({ queryKey: ['files'] });
        queryClient.invalidateQueries({ queryKey: ['api-health'] });
        globalSearchActiveRef.current = false;
        setSearchResults([]);
    }, [queryClient]);

    const handleShare = useCallback((file: TelegramFile) => {
        if (!shareReady) {
            toast.error(shareBlockedMessage || transferBlockedMessage);
            return;
        }
        setShareFile(file);
    }, [shareReady, shareBlockedMessage, transferBlockedMessage]);

    const handleEnter = useCallback(() => {
        if (selectedIds.length === 1) {
            const selected = displayedFiles.find(f => f.id === selectedIds[0]);
            if (selected) {
                if (selected.type === 'folder') {
                    setActiveFolderId(selected.id);
                } else {
                    handlePreview(selected, displayedFiles);
                }
            }
        }
    }, [selectedIds, displayedFiles, setActiveFolderId]);

    useKeyboardShortcuts({
        onSelectAll: handleSelectAll,
        onDelete: handleKeyboardDelete,
        onEscape: handleEscape,
        onSearch: handleFocusSearch,
        onEnter: handleEnter,
        enabled: !previewFile && !playingFile && !pdfFile && !showMoveModal && !shareFile && !showSettings,
        transferEnabled: sessionOnline,
        deleteEnabled: deleteReady,
        previewEnabled: previewReady,
    });


    useEffect(() => {
        setSelectedIds([]);
        setShowMoveModal(false);
        setSearchTerm("");
        setSearchResults([]);
        setPreviewFile(null);
        setPlayingFile(null);
        setPdfFile(null);
        setPreviewContextFiles([]);
        setPreviewContextIndex(-1);
    }, [activeFolderId]);


    useEffect(() => {
        if (!isGlobalSearchActive(searchTerm)) {
            globalSearchActiveRef.current = false;
            setSearchResults([]);
            return;
        }
        if (!serviceReady) {
            setSearchResults([]);
            return;
        }

        const timer = setTimeout(async () => {
            setIsSearching(true);
            try {
                if (shouldRebuildIndexBeforeGlobalSearch({
                    botIndexMode: botIndexReady,
                    wasActive: globalSearchActiveRef.current,
                    term: searchTerm,
                })) {
                    globalSearchActiveRef.current = true;
                    try {
                        const rebuilt = await invoke<{ folders_scanned: number; files_indexed: number }>(
                            'cmd_rebuild_file_index',
                            { folderIds: buildRebuildFolderIds(folders) },
                        );
                        if (rebuilt.files_indexed > 0) {
                            toast.info(
                                `索引重建完成: ${rebuilt.files_indexed} 个文件，${rebuilt.folders_scanned} 个文件夹`,
                            );
                        }
                    } catch (rebuildErr) {
                        toast.info(formatIndexRebuildBackgroundFailureMessage(rebuildErr));
                    }
                } else {
                    globalSearchActiveRef.current = true;
                }
                const results = await invoke<TelegramFile[]>('cmd_search_global', { query: searchTerm });
                setSearchResults(results);
            } catch (e) {
                const errMsg = String(e);
                toast.error(`搜索失败: ${errMsg}`);
                setSearchResults([]);
                if (isSessionLostError(errMsg)) {
                    onSessionError(errMsg);
                }
            } finally {
                setIsSearching(false);
            }
        }, 500);

        return () => clearTimeout(timer);
    }, [searchTerm, serviceReady, botIndexReady, onSessionError, folders]);




    const handleFileClick = (e: React.MouseEvent, id: number) => {
        e.stopPropagation();
        if (e.metaKey || e.ctrlKey) {
            setSelectedIds(ids => ids.includes(id) ? ids.filter(i => i !== id) : [...ids, id]);
        } else {
            setSelectedIds([id]);
        }
    }

    const handleToggleSelection = useCallback((id: number) => {
        setSelectedIds(ids => ids.includes(id) ? ids.filter(i => i !== id) : [...ids, id]);
    }, []);

    const handlePreview = (file: TelegramFile, orderedFiles?: TelegramFile[]) => {
        if (file.type !== 'folder' && !previewReady) {
            toast.error(previewBlockedMessage || transferBlockedMessage);
            return;
        }
        const contextFiles = (orderedFiles || displayedFiles).filter((f) => f.type !== 'folder');
        const contextIndex = contextFiles.findIndex((f) => f.id === file.id);

        setPreviewContextFiles(contextFiles);
        setPreviewContextIndex(contextIndex);

        const isMedia = isMediaFile(file.name);
        const isPdf = isPdfFile(file.name);

        if (isMedia) {
            setPlayingFile(file);
            setPreviewFile(null);
            setPdfFile(null);
        } else if (isPdf) {
            setPdfFile(file);
            setPreviewFile(null);
            setPlayingFile(null);
        } else {
            setPreviewFile(file);
            setPlayingFile(null);
            setPdfFile(null);
        }
    };

    const navigatePreview = useCallback((step: 1 | -1) => {
        if (!previewReady) {
            toast.error(previewBlockedMessage || transferBlockedMessage);
            return;
        }
        if (previewContextFiles.length === 0) return;

        const currentFileId = previewFile?.id ?? playingFile?.id ?? pdfFile?.id;
        if (!currentFileId) return;

        const currentIndex = previewContextFiles.findIndex((f) => f.id === currentFileId);
        if (currentIndex === -1) return;

        const nextIndex = (currentIndex + step + previewContextFiles.length) % previewContextFiles.length;
        const nextFile = previewContextFiles[nextIndex];
        if (!nextFile) return;

        setPreviewContextIndex(nextIndex);

        const isMedia = isMediaFile(nextFile.name);
        const isPdf = isPdfFile(nextFile.name);

        if (isMedia) {
            setPlayingFile(nextFile);
            setPreviewFile(null);
            setPdfFile(null);
        } else if (isPdf) {
            setPdfFile(nextFile);
            setPreviewFile(null);
            setPlayingFile(null);
        } else {
            setPreviewFile(nextFile);
            setPlayingFile(null);
            setPdfFile(null);
        }
    }, [previewContextFiles, previewFile, playingFile, pdfFile, previewReady, previewBlockedMessage, transferBlockedMessage]);

    useEffect(() => {
        if (!previewReady) {
            setPreviewFile(null);
            setPlayingFile(null);
            setPdfFile(null);
        }
        if (!shareReady) {
            setShareFile(null);
        }
    }, [previewReady, shareReady]);

    const handleNextPreview = useCallback(() => {
        navigatePreview(1);
    }, [navigatePreview]);

    const handlePrevPreview = useCallback(() => {
        navigatePreview(-1);
    }, [navigatePreview]);

    const previewNeighborFiles = useCallback(() => {
        if (previewContextFiles.length === 0) {
            return { nextFile: null as TelegramFile | null, prevFile: null as TelegramFile | null };
        }

        const currentFileId = previewFile?.id ?? playingFile?.id ?? pdfFile?.id;
        if (!currentFileId) {
            return { nextFile: null as TelegramFile | null, prevFile: null as TelegramFile | null };
        }

        const currentIdx = previewContextFiles.findIndex((f) => f.id === currentFileId);
        if (currentIdx === -1) {
            return { nextFile: null as TelegramFile | null, prevFile: null as TelegramFile | null };
        }

        const nextIdx = (currentIdx + 1) % previewContextFiles.length;
        const prevIdx = (currentIdx - 1 + previewContextFiles.length) % previewContextFiles.length;

        return {
            nextFile: previewContextFiles[nextIdx] || null,
            prevFile: previewContextFiles[prevIdx] || null,
        };
    }, [previewContextFiles, previewFile, playingFile, pdfFile]);

    const handleDropOnFolder = async (e: React.DragEvent, targetFolderId: number | null) => {
        e.preventDefault();
        e.stopPropagation();

        if (!sessionOnline) {
            toast.error(transferBlockedMessage);
            return;
        }
        if (!bulkMoveAllowed) {
            toast.error(bulkMoveBlockedMessage(transportMode, 'desktop'));
            return;
        }

        const dataTransferFileId = e.dataTransfer.getData("application/x-telegram-file-id");

        if (activeFolderId === targetFolderId) return;

        const fileId = internalDragRef.current || (dataTransferFileId ? parseInt(dataTransferFileId) : null);

        if (fileId) {
            const idsToMove = selectedIds.includes(fileId) ? selectedIds : [fileId];
            const groups = planMoveGroups(idsToMove, displayedFiles, activeFolderId, targetFolderId);
            const { moved, movedOldIds, mergedPayload, failures } = await executeMoveGroups(
                groups,
                targetFolderId,
            );

            if (movedOldIds.length > 0) {
                queryClient.invalidateQueries({ queryKey: ['files'] });
                if (mergedPayload) handleFilesMoved(mergedPayload);
                setSelectedIds(pruneSelectedIdsAfterDelete(selectedIds, movedOldIds));
            }

            if (moved > 0) toast.success(`已移动 ${moved} 个文件`);
            if (failures.length > 0) {
                const detail = failures.length === groups.length
                    ? failures[0]
                    : `部分移动失败（${failures.length}/${groups.length}）：${failures[0]}`;
                toast.error(`移动文件失败: ${detail}`);
                const sessionErr = failures.find((f) => isSessionLostError(f));
                if (sessionErr) onSessionError(sessionErr);
            } else if (moved === 0) {
                toast.info('文件已在此文件夹中');
            }

            setInternalDragFileId(null);
        }
    }

    const handleFolderDeleteWithCleanup = useCallback(async (folderId: number, folderName: string) => {
        await handleFolderDelete(folderId, folderName);
        const affectsOpenFile =
            activeFolderId === folderId ||
            fileBelongsToFolder(previewFile, folderId, activeFolderId) ||
            fileBelongsToFolder(playingFile, folderId, activeFolderId) ||
            fileBelongsToFolder(pdfFile, folderId, activeFolderId) ||
            fileBelongsToFolder(shareFile, folderId, activeFolderId);
        if (affectsOpenFile) {
            closePreviewState();
            setShareFile(null);
        }
    }, [
        handleFolderDelete,
        activeFolderId,
        previewFile,
        playingFile,
        pdfFile,
        shareFile,
        closePreviewState,
    ]);

    const currentFolderName = activeFolderId === null
        ? "Saved Messages"
        : folders.find(f => f.id === activeFolderId)?.name || "Folder";


    const handleRootDragOver = (e: React.DragEvent) => {
        if (internalDragRef.current) {
            e.preventDefault();
            e.stopPropagation();
            e.dataTransfer.dropEffect = 'move';
        }
    };

    const handleRootDragEnter = (e: React.DragEvent) => {
        if (internalDragRef.current) {
            e.preventDefault();
            e.stopPropagation();
            e.dataTransfer.dropEffect = 'move';
        }
    };

    const previewNeighbors = previewNeighborFiles();

    return (
        <div
            className="flex h-screen w-full overflow-hidden bg-telegram-bg relative"
            onClick={() => setSelectedIds([])}
            onDragOver={handleRootDragOver}
            onDragEnter={handleRootDragEnter}
        >

            <ExternalDropBlocker
                onUploadPaths={enqueueUploadPaths}
                onUploadClick={handleManualUpload}
                uploadEnabled={sessionOnline}
                onUploadBlocked={() => toast.error('Telegram 会话未就绪，无法上传')}
            />

            <AnimatePresence>
                {showMoveModal && (
                    <MoveToFolderModal
                        folders={folders}
                        onClose={() => setShowMoveModal(false)}
                        onSelect={(id) => {
                            handleBulkMove(id, () => {
                                setShowMoveModal(false);
                            });
                        }}
                        activeFolderId={activeFolderId}
                        key="move-modal"
                    />
                )}
                {playingFile && (
                    <MediaPlayer
                        file={playingFile}
                        onClose={() => setPlayingFile(null)}
                        onNext={handleNextPreview}
                        onPrev={handlePrevPreview}
                        currentIndex={previewContextIndex}
                        totalItems={previewContextFiles.length}
                        activeFolderId={activeFolderId}
                        key="media-player"
                    />
                )}
                {pdfFile && (
                    <PdfViewer
                        file={pdfFile}
                        onClose={() => setPdfFile(null)}
                        onNext={handleNextPreview}
                        onPrev={handlePrevPreview}
                        currentIndex={previewContextIndex}
                        totalItems={previewContextFiles.length}
                        activeFolderId={activeFolderId}
                        key="pdf-viewer"
                    />
                )}
            </AnimatePresence>

            <Sidebar
                folders={folders}
                activeFolderId={activeFolderId}
                setActiveFolderId={setActiveFolderId}
                onDrop={handleDropOnFolder}
                onDelete={handleFolderDeleteWithCleanup}
                onCreate={handleCreateFolder}
                isSyncing={isSyncing}
                isConnected={isConnected}
                connectionStatus={connectionStatus}
                onSync={handleSyncFolders}
                onLogout={handleLogout}
                bandwidth={bandwidth || null}
                collapsed={sidebarCollapsed}
                onToggle={() => setSidebarCollapsed(!sidebarCollapsed)}
            />

            {/* Mobile sidebar overlay */}
            {!sidebarCollapsed && (
                <div
                    className="fixed inset-0 bg-black/50 z-[80] md:hidden"
                    onClick={() => setSidebarCollapsed(true)}
                />
            )}

            <main className="flex-1 flex flex-col" onClick={(e) => { if (e.target === e.currentTarget) setSelectedIds([]); }}>
                {/* Mobile hamburger button */}
                <button
                    onClick={() => setSidebarCollapsed(false)}
                    className="md:hidden fixed top-4 left-4 z-[70] p-2 bg-telegram-surface border border-telegram-border rounded-lg text-telegram-text hover:bg-telegram-hover"
                    aria-label="Open sidebar"
                >
                    <Menu className="w-5 h-5" />
                </button>
                <TopBar
                    currentFolderName={currentFolderName}
                    selectedIds={selectedIds}
                    sessionOnline={sessionOnline}
                    downloadReady={downloadReady}
                    deleteReady={deleteReady}
                    downloadBlockedMessage={downloadBlockedMessage}
                    transferBlockedMessage={transferBlockedMessage}
                    bulkMoveAllowed={bulkMoveAllowed}
                    bulkMoveBlockedMessage={bulkMoveBlockedMessage(transportMode, 'desktop')}
                    onNavigateHome={() => { setActiveFolderId(null); setSearchTerm(''); }}
                    onShowMoveModal={() => setShowMoveModal(true)}
                    onBulkDownload={handleBulkDownload}
                    onBulkDelete={handleBulkDelete}
                    onDownloadFolder={handleDownloadFolder}
                    viewMode={viewMode}
                    setViewMode={setViewMode}
                    searchTerm={searchTerm}
                    onSearchChange={setSearchTerm}
                    onSettingsClick={() => setShowSettings(true)}
                />
                {(!transferReady || botIndexReady) && (
                    <div className="mx-4 md:mx-6 mt-2 px-4 py-2 rounded-lg bg-yellow-500/10 border border-yellow-500/30 text-sm text-yellow-200/90">
                        {botIndexReady && !transferReady ? (
                            <>
                                Bot 模式已就绪 — 可浏览、搜索、预览、下载、分享与删除索引条目；上传与移动仍需 User 会话。
                            </>
                        ) : (
                            <>
                                {transferBlockedMessage}
                                {connectionStatus === 'session_lost' && ' — uploads, downloads, and file moves are disabled until you sign in again.'}
                                {connectionStatus === 'network_offline' && ' — check your network connection.'}
                                {connectionStatus === 'checking' && ' — verifying connection…'}
                            </>
                        )}
                    </div>
                )}
                {searchTerm.length > 2 && (
                    <div className="px-6 pt-4 pb-0">
                        <h2 className="text-sm font-medium text-telegram-subtext">
                            Search Results for <span className="text-telegram-primary">"{searchTerm}"</span>
                        </h2>
                    </div>
                )}
                <FileExplorer
                    folders={folders}
                    files={displayedFiles}
                    loading={isLoading || isSearching}
                    error={error}
                    viewMode={viewMode}
                    selectedIds={selectedIds}
                    activeFolderId={activeFolderId}
                    sessionOnline={sessionOnline}
                    downloadReady={downloadReady}
                    previewReady={previewReady}
                    shareReady={shareReady}
                    deleteReady={deleteReady}
                    transferBlockedMessage={transferBlockedMessage}
                    downloadBlockedMessage={downloadBlockedMessage}
                    previewBlockedMessage={previewBlockedMessage}
                    shareBlockedMessage={shareBlockedMessage}
                    isGlobalSearch={searchTerm.length > 2}
                    onFileClick={handleFileClick}
                    onDelete={handleDelete}
                    onDownload={(id, name) => {
                        const file = displayedFiles.find((f) => f.id === id);
                        queueDownload(id, name, resolveFileFolderId(file ?? {}, activeFolderId));
                    }}
                    onPreview={handlePreview}
                    onManualUpload={handleManualUpload}
                    onFolderUpload={handleFolderUpload}
                    showFolderUpload={settings.zipFolders}
                    onSelectionClear={() => setSelectedIds([])}
                    onToggleSelection={handleToggleSelection}
                    onDrop={handleDropOnFolder}
                    onDragStart={(fileId) => setInternalDragFileId(fileId)}
                    onDragEnd={() => setTimeout(() => setInternalDragFileId(null), 50)}
                    onShare={handleShare}
                />
            </main>

            {previewFile && (
                <PreviewModal
                    file={previewFile}
                    activeFolderId={activeFolderId}
                    onClose={() => setPreviewFile(null)}
                    onNext={handleNextPreview}
                    onPrev={handlePrevPreview}
                    currentIndex={previewContextIndex}
                    totalItems={previewContextFiles.length}
                    nextFile={previewNeighbors.nextFile}
                    prevFile={previewNeighbors.prevFile}
                />
            )}


            <UploadQueue
                items={uploadQueue}
                onClearFinished={clearUploads}
                onCancelAll={cancelUploads}
                onCancelItem={cancelUploadItem}
                onRetryItem={retryUploadItem}
            />
            <DownloadQueue
                items={downloadQueue}
                onClearFinished={clearDownloads}
                onCancelAll={cancelDownloads}
                onCancelItem={cancelDownloadItem}
                onRetryItem={retryDownloadItem}
            />

            <SettingsModal
                isOpen={showSettings}
                onClose={() => setShowSettings(false)}
                sessionOnline={sessionOnline}
                shareReady={shareReady}
                transferBlockedMessage={transferBlockedMessage}
                shareBlockedMessage={shareBlockedMessage}
                onTransportSwitched={handleTransportSwitched}
            />

            {shareFile && (
                <ShareDialog
                    file={shareFile}
                    activeFolderId={activeFolderId}
                    shareReady={shareReady}
                    shareBlockedMessage={shareBlockedMessage || transferBlockedMessage}
                    onSessionError={onSessionError}
                    onClose={() => setShareFile(null)}
                />
            )}
        </div>
    );
}
