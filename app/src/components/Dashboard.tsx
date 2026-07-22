import { useState, useEffect, useCallback } from 'react';
import { AnimatePresence } from 'framer-motion';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { Menu } from 'lucide-react';

import {
    formatBytes, resolveFileFolderId,
    fileBelongsToFolder,
} from '../utils';
import { formatSize, formatTime } from '../lib/utils';
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
import { BatchProgressPanel, TaskItem } from './dashboard/BatchProgressPanel';
import { ShortcutsHelp } from './dashboard/ShortcutsHelp';
import { MobileTabBar } from './dashboard/MobileTabBar';

// Hooks
import { useTelegramConnection } from '../hooks/useTelegramConnection';
import { useFileOperations } from '../hooks/useFileOperations';
import { useFileUpload } from '../hooks/useFileUpload';
import { useFileDownload } from '../hooks/useFileDownload';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';
import { useSettings } from '../context/SettingsContext';
import { usePreviewManager } from '../hooks/usePreviewManager';
import { useGlobalSearch } from '../hooks/useGlobalSearch';
import { useDragAndDrop } from '../hooks/useDragAndDrop';
import {
    canDownloadFiles,
    canPreviewFiles,
    canShareFiles,
    canTransferFiles,
    connectionStatusLabel,
    isBotIndexReady,
    isServiceReady,
} from '../types/connection';
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

    const [selectedIds, setSelectedIds] = useState<number[]>([]);
    const [showMoveModal, setShowMoveModal] = useState(false);
    const [showSettings, setShowSettings] = useState(false);
    const [sidebarCollapsed, setSidebarCollapsed] = useState(true); // Mobile: collapsed by default
    const [showShortcutsHelp, setShowShortcutsHelp] = useState(false);
    const [mobileTab, setMobileTab] = useState<'files' | 'search' | 'settings'>('files');

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

    const {
        searchTerm, setSearchTerm, searchResults, isSearching,
        resetSearch, handleFilesMoved: searchHandleFilesMoved,
        handleFilesRemoved: searchHandleFilesRemoved,
    } = useGlobalSearch(serviceReady, botIndexReady, folders, onSessionError);

    const displayedFiles = searchTerm.length > 2
        ? searchResults
        : allFiles.filter((f: TelegramFile) => f.name.toLowerCase().includes(searchTerm.toLowerCase()));

    const {
        previewFile, playingFile, pdfFile, shareFile,
        previewContextFiles, previewContextIndex,
        handlePreview, handleNextPreview, handlePrevPreview,
        previewNeighborFiles, closePreviewState, closePreviewIfRemoved,
        setShareFile, handleFilesRemoved: previewHandleFilesRemoved,
        handleFilesMoved: previewHandleFilesMoved,
        setPreviewContextFiles, setPreviewContextIndex,
        setPreviewFile, setPlayingFile, setPdfFile,
    } = usePreviewManager(displayedFiles, previewReady, previewBlockedMessage, transferBlockedMessage);

    const {
        setInternalDragFileId,
        handleDropOnFolder, handleRootDragOver, handleRootDragEnter,
    } = useDragAndDrop();

    const transportMode = apiHealth?.transport_mode;
    const bulkMoveAllowed =
        transportMode == null || canBulkMoveInTransportMode(transportMode);

    const handleFilesRemoved = useCallback((removedIds: number[]) => {
        closePreviewIfRemoved(removedIds);
        searchHandleFilesRemoved(removedIds);
        previewHandleFilesRemoved(removedIds);
        window.dispatchEvent(
            new CustomEvent('td-shares-invalidate', { detail: { messageIds: removedIds } }),
        );
    }, [closePreviewIfRemoved, searchHandleFilesRemoved, previewHandleFilesRemoved]);

    const handleFilesMoved = useCallback((payload: MoveFilesPayload) => {
        searchHandleFilesMoved(payload);
        previewHandleFilesMoved(payload);
        if (searchTerm.length > 2 && payload.oldIds.length > 0) {
            toast.success('文件已移动 — 搜索结果已更新');
        }
    }, [searchTerm, searchHandleFilesMoved, previewHandleFilesMoved]);

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
        transportMode,
        onFilesRemoved: handleFilesRemoved,
        onFilesMoved: handleFilesMoved,
    });

    const { uploadQueue, enqueueUploadPaths, handleManualUpload, handleFolderUpload, cancelAll: cancelUploads, cancelItem: cancelUploadItem, retryItem: retryUploadItem, clearFinished: clearUploads } = useFileUpload(activeFolderId, store, { onSessionError, ...transferOpts });

    // Map uploadQueue QueueItems to BatchProgressPanel TaskItems
    const uploadTasks: TaskItem[] = uploadQueue.map(item => {
        const fileName = item.path.split(/[/\\]/).pop() || item.path;
        const statusMap: Record<string, TaskItem['status']> = {
            pending: 'pending',
            uploading: 'running',
            success: 'completed',
            error: 'failed',
            cancelled: 'cancelled',
        };
        return {
            id: item.id,
            name: fileName,
            status: statusMap[item.status] || 'pending',
            percent: item.progress ?? 0,
            speed: item.speedBytesPerSec ? `${formatSize(item.speedBytesPerSec)}/s` : undefined,
            remaining: item.speedBytesPerSec && item.uploadedBytes && item.totalBytes
                ? formatTime((item.totalBytes - item.uploadedBytes) / item.speedBytesPerSec)
                : undefined,
        };
    });

    const uploadLiveSummary = (() => {
        const active = uploadQueue.filter((item) => item.status === 'pending' || item.status === 'uploading').length;
        const failed = uploadQueue.filter((item) => item.status === 'error' || item.status === 'cancelled').length;
        if (active > 0) return `${active} 个上传任务正在处理`;
        if (failed > 0) return `${failed} 个上传任务需要重试或人工处理`;
        if (uploadQueue.some((item) => item.status === 'success')) return '上传任务已完成，可在文件列表生成或复制直链';
        return '';
    })();
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
    }, [shareFile, showSettings, showMoveModal, setSearchTerm, setPreviewFile, setPlayingFile, setPdfFile, setShareFile]);

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
        resetSearch();
    }, [queryClient, resetSearch]);

    const handleShare = useCallback((file: TelegramFile) => {
        if (!shareReady) {
            toast.error(shareBlockedMessage || transferBlockedMessage);
            return;
        }
        setShareFile(file);
    }, [shareReady, shareBlockedMessage, transferBlockedMessage, setShareFile]);

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
    }, [selectedIds, displayedFiles, setActiveFolderId, handlePreview]);

    useKeyboardShortcuts({
        onSelectAll: handleSelectAll,
        onDelete: handleKeyboardDelete,
        onEscape: handleEscape,
        onSearch: handleFocusSearch,
        onEnter: handleEnter,
        onToggleHelp: () => setShowShortcutsHelp(prev => !prev),
        enabled: !previewFile && !playingFile && !pdfFile && !showMoveModal && !shareFile && !showSettings,
        transferEnabled: sessionOnline,
        deleteEnabled: deleteReady,
        previewEnabled: previewReady,
    });

    useEffect(() => {
        setSelectedIds([]);
        setShowMoveModal(false);
        resetSearch();
        setPreviewFile(null);
        setPlayingFile(null);
        setPdfFile(null);
        setPreviewContextFiles([]);
        setPreviewContextIndex(-1);
    }, [activeFolderId, resetSearch, setPreviewFile, setPlayingFile, setPdfFile, setPreviewContextFiles, setPreviewContextIndex]);

    useEffect(() => {
        if (!previewReady) {
            setPreviewFile(null);
            setPlayingFile(null);
            setPdfFile(null);
        }
        if (!shareReady) {
            setShareFile(null);
        }
    }, [previewReady, shareReady, setPreviewFile, setPlayingFile, setPdfFile, setShareFile]);

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
        setShareFile,
    ]);

    const currentFolderName = activeFolderId === null
        ? "Saved Messages"
        : folders.find(f => f.id === activeFolderId)?.name || "Folder";

    const previewNeighbors = previewNeighborFiles();

    const handleDropOnFolderWrapper = useCallback(async (e: React.DragEvent, targetFolderId: number | null) => {
        await handleDropOnFolder(e, targetFolderId, {
            activeFolderId,
            selectedIds,
            displayedFiles,
            sessionOnline,
            transferBlockedMessage,
            bulkMoveAllowed,
            bulkMoveBlockedMessage: bulkMoveBlockedMessage(transportMode, 'desktop'),
            onFilesMoved: handleFilesMoved,
            onSessionError,
            invalidateFiles: () => queryClient.invalidateQueries({ queryKey: ['files'] }),
            setSelectedIds,
        });
    }, [activeFolderId, selectedIds, displayedFiles, sessionOnline, transferBlockedMessage, bulkMoveAllowed, transportMode, handleFilesMoved, onSessionError, queryClient, setSelectedIds, handleDropOnFolder]);

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

    const { data: bandwidth } = useQuery({
        queryKey: ['bandwidth'],
        queryFn: () => invoke<BandwidthStats>('cmd_get_bandwidth'),
        refetchInterval: 5000,
        enabled: !!store && transferReady,
    });

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

            <ShortcutsHelp
                open={showShortcutsHelp}
                onClose={() => setShowShortcutsHelp(false)}
            />

            <Sidebar
                folders={folders}
                activeFolderId={activeFolderId}
                setActiveFolderId={setActiveFolderId}
                onDrop={handleDropOnFolderWrapper}
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
                    className="fixed inset-0 bg-black/50 z-[80] md:hidden animate-in fade-in duration-200"
                    onClick={() => setSidebarCollapsed(true)}
                    role="presentation"
                    aria-hidden="true"
                />
            )}

            <main className="flex-1 flex flex-col md:pb-0 pb-16" role="main" aria-label="File explorer" onClick={(e) => { if (e.target === e.currentTarget) setSelectedIds([]); }}>
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
                    <div className="mx-4 md:mx-6 mt-2 px-4 py-2 rounded-lg bg-yellow-500/10 border border-yellow-500/30 text-sm text-yellow-200/90" role="status" aria-live="polite">
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
                    onDrop={handleDropOnFolderWrapper}
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


            <div className="sr-only" role="status" aria-live="polite" aria-atomic="true">
                {uploadLiveSummary}
            </div>
            <BatchProgressPanel
                tasks={uploadTasks}
                onCancel={(id) => cancelUploadItem(id)}
                onCancelAll={cancelUploads}
            />

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

            {/* Mobile bottom tab bar */}
            <MobileTabBar
                activeTab={mobileTab}
                onTabChange={(tab) => {
                    setMobileTab(tab);
                    if (tab === 'settings') setShowSettings(true);
                    if (tab === 'search') handleFocusSearch();
                }}
                onOpenSidebar={() => setSidebarCollapsed(false)}
            />
        </div>
    );
}
