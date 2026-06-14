import { HardDrive, LayoutGrid, Sun, Moon, Settings } from 'lucide-react';
import { useTheme } from '../../context/ThemeContext';

interface TopBarProps {
    currentFolderName: string;
    selectedIds: number[];
    sessionOnline: boolean;
    downloadReady?: boolean;
    deleteReady?: boolean;
    downloadBlockedMessage?: string;
    transferBlockedMessage?: string;
    bulkMoveAllowed?: boolean;
    bulkMoveBlockedMessage?: string;
    onNavigateHome: () => void;
    onShowMoveModal: () => void;
    onBulkDownload: () => void;
    onBulkDelete: () => void;
    onDownloadFolder: () => void;
    viewMode: 'grid' | 'list';
    setViewMode: (mode: 'grid' | 'list') => void;
    searchTerm: string;
    onSearchChange: (term: string) => void;
    onSettingsClick: () => void;
}

export function TopBar({
    currentFolderName, selectedIds, sessionOnline,
    downloadReady = sessionOnline, deleteReady = sessionOnline,
    downloadBlockedMessage, transferBlockedMessage, bulkMoveAllowed = true, bulkMoveBlockedMessage,
    onNavigateHome, onShowMoveModal, onBulkDownload, onBulkDelete,
    onDownloadFolder, viewMode, setViewMode, searchTerm, onSearchChange, onSettingsClick
}: TopBarProps) {
    const { theme, toggleTheme } = useTheme();
    const blockedTitle = transferBlockedMessage || 'Session not ready';
    const downloadBlockedTitle = downloadBlockedMessage || blockedTitle;
    const moveBlockedTitle = !sessionOnline ? blockedTitle : (!bulkMoveAllowed ? (bulkMoveBlockedMessage || 'Bulk move not available') : undefined);
    const moveEnabled = sessionOnline && bulkMoveAllowed;

    const guardDownload = (action: () => void) => {
        if (!downloadReady) return;
        action();
    };

    const guardDelete = (action: () => void) => {
        if (!deleteReady) return;
        action();
    };

    const guardMove = (action: () => void) => {
        if (!moveEnabled) return;
        action();
    };

    return (
        <header className="h-14 border-b border-telegram-border flex items-center px-4 justify-between bg-telegram-surface/80 backdrop-blur-md sticky top-0 z-10" onClick={e => e.stopPropagation()}>
            <div className="flex items-center gap-4">
                <div className="flex items-center text-sm breadcrumbs text-telegram-subtext select-none">
                    <span className="hover:text-telegram-text cursor-pointer transition-colors" onClick={onNavigateHome} role="button" tabIndex={0} onKeyDown={(e) => e.key === 'Enter' && onNavigateHome()}>Start</span>
                    <span className="mx-2">/</span>
                    <span className="text-telegram-text font-medium">{currentFolderName}</span>
                </div>
            </div>

            <div className="flex-1 max-w-md mx-4">
                <input
                    type="text"
                    placeholder="Search files..."
                    className="w-full bg-telegram-hover border border-telegram-border rounded-lg px-3 py-1.5 text-sm text-telegram-text placeholder:text-telegram-subtext focus:outline-none focus:border-telegram-primary/50 transition-colors"
                    value={searchTerm}
                    onChange={(e) => onSearchChange(e.target.value)}
                />
            </div>

            <div className="flex items-center gap-2">
                {selectedIds.length > 0 && (
                    <div className="flex items-center gap-2 mr-4 animate-in fade-in slide-in-from-top-2">
                        <span className="text-xs text-telegram-subtext mr-2">{selectedIds.length} Selected</span>
                        <button
                            onClick={() => guardMove(onShowMoveModal)}
                            disabled={!moveEnabled}
                            title={moveBlockedTitle}
                            className="px-3 py-1.5 bg-telegram-primary/20 hover:bg-telegram-primary/30 text-telegram-primary rounded-md text-xs transition font-medium disabled:opacity-40 disabled:cursor-not-allowed"
                        >
                            Move to...
                        </button>
                        <button
                            onClick={() => guardDownload(onBulkDownload)}
                            disabled={!downloadReady}
                            title={!downloadReady ? downloadBlockedTitle : undefined}
                            className="px-3 py-1.5 bg-telegram-hover hover:bg-telegram-border rounded-md text-xs text-telegram-text transition disabled:opacity-40 disabled:cursor-not-allowed"
                        >
                            Download Selected
                        </button>
                        <button
                            onClick={() => guardDelete(onBulkDelete)}
                            disabled={!deleteReady}
                            title={!deleteReady ? blockedTitle : undefined}
                            className="px-3 py-1.5 bg-red-500/10 hover:bg-red-500/20 text-red-400 rounded-md text-xs transition disabled:opacity-40 disabled:cursor-not-allowed"
                        >
                            Delete
                        </button>
                    </div>
                )}

                <button
                    onClick={() => guardDownload(onDownloadFolder)}
                    disabled={!downloadReady}
                    title={!downloadReady ? downloadBlockedTitle : 'Download Folder'}
                    className="p-2 hover:bg-telegram-hover rounded-md text-telegram-subtext hover:text-telegram-text transition group relative disabled:opacity-40 disabled:cursor-not-allowed"
                >
                    <HardDrive className="w-5 h-5" />
                    <span className="absolute -bottom-8 left-1/2 -translate-x-1/2 text-[10px] bg-telegram-surface border border-telegram-border px-2 py-1 rounded opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-50 shadow-lg">
                        Download All Files
                    </span>
                </button>

                <button
                    onClick={() => setViewMode(viewMode === 'grid' ? 'list' : 'grid')}
                    className="p-2 hover:bg-telegram-hover rounded-md text-telegram-subtext hover:text-telegram-text transition relative group"
                    title="Toggle Layout"
                >
                    <LayoutGrid className="w-5 h-5" />
                    <span className="absolute -bottom-8 left-1/2 -translate-x-1/2 text-[10px] bg-telegram-surface border border-telegram-border px-2 py-1 rounded opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-50 shadow-lg">
                        {viewMode === 'grid' ? 'Switch to List' : 'Switch to Grid'}
                    </span>
                </button>

                <div className="w-px h-6 bg-telegram-border mx-1"></div>

                <button
                    onClick={onSettingsClick}
                    className="p-2 hover:bg-telegram-hover rounded-md text-telegram-subtext hover:text-telegram-text transition relative group"
                    title="Settings"
                >
                    <Settings className="w-5 h-5" />
                    <span className="absolute -bottom-8 left-1/2 -translate-x-1/2 text-[10px] bg-telegram-surface border border-telegram-border px-2 py-1 rounded opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-50 shadow-lg">
                        Settings
                    </span>
                </button>

                <button
                    onClick={toggleTheme}
                    className="p-2 hover:bg-telegram-hover rounded-md text-telegram-subtext hover:text-telegram-text transition relative group"
                    title={theme === 'dark' ? 'Switch to Light Mode' : 'Switch to Dark Mode'}
                >
                    {theme === 'dark' ? <Sun className="w-5 h-5" /> : <Moon className="w-5 h-5" />}
                    <span className="absolute -bottom-8 left-1/2 -translate-x-1/2 text-[10px] bg-telegram-surface border border-telegram-border px-2 py-1 rounded opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap z-50 shadow-lg">
                        {theme === 'dark' ? 'Light Mode' : 'Dark Mode'}
                    </span>
                </button>
            </div>
        </header>
    )
}
