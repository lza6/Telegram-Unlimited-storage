import { useState } from 'react';
import { HardDrive, Folder, Plus, RefreshCw, LogOut, X } from 'lucide-react';
import { SidebarItem } from './SidebarItem';
import { BandwidthWidget } from './BandwidthWidget';
import { TelegramFolder, BandwidthStats } from '../../types';
import type { ConnectionStatus } from '../../types/connection';
import { connectionStatusLabel } from '../../types/connection';

interface SidebarProps {
    folders: TelegramFolder[];
    activeFolderId: number | null;
    setActiveFolderId: (id: number | null) => void;
    onDrop: (e: React.DragEvent, folderId: number | null) => void;
    onDelete: (id: number, name: string) => void;
    onCreate: (name: string) => Promise<void>;
    isSyncing: boolean;
    isConnected: boolean;
    connectionStatus?: ConnectionStatus;
    onSync: () => void;
    onLogout: () => void;
    bandwidth: BandwidthStats | null;
    collapsed?: boolean;
    onToggle?: () => void;
}

export function Sidebar({
    folders, activeFolderId, setActiveFolderId, onDrop, onDelete, onCreate,
    isSyncing, isConnected, connectionStatus, onSync, onLogout, bandwidth, collapsed, onToggle
}: SidebarProps) {
    const status: ConnectionStatus = connectionStatus ?? (isConnected ? 'online' : 'session_lost');
    const dropEnabled = status === 'online';
    const statusDotClass =
        status === 'online'
            ? 'bg-green-500 animate-pulse'
            : status === 'session_lost'
              ? 'bg-yellow-500'
              : 'bg-red-500';
    const [showNewFolderInput, setShowNewFolderInput] = useState(false);
    const [newFolderName, setNewFolderName] = useState("");

    const submitCreate = async () => {
        if (!newFolderName.trim()) return;
        try {
            await onCreate(newFolderName);
            setNewFolderName("");
            setShowNewFolderInput(false);
        } catch {
            // handled by parent
        }
    }

    return (
        <aside className={`${collapsed ? '-translate-x-full md:translate-x-0' : 'translate-x-0'} md:flex w-64 bg-telegram-surface border-r border-telegram-border flex-col fixed md:relative inset-y-0 left-0 z-[90] transition-transform duration-300 ease-out`} onClick={e => e.stopPropagation()}>
            {/* Mobile close button */}
            {onToggle && (
                <button
                    onClick={onToggle}
                    className="md:hidden absolute top-4 right-4 p-1 text-telegram-subtext hover:text-telegram-text"
                    aria-label="Close sidebar"
                >
                    <X className="w-5 h-5" />
                </button>
            )}
            <div className="p-4 flex items-center gap-2">
                <img src="/logo.svg" className="w-8 h-8 drop-shadow-lg" alt="Logo" />
                <span className="font-bold text-lg text-telegram-text tracking-tight">Telegram Drive</span>
            </div>

            {/* Scrollable folder list */}
            <nav role="navigation" aria-label="Folder navigation" className="flex-1 px-2 py-4 space-y-1 overflow-y-auto min-h-0">
                <SidebarItem
                    icon={HardDrive}
                    label="Saved Messages"
                    active={activeFolderId === null}
                    onClick={() => setActiveFolderId(null)}
                    onDrop={(e: React.DragEvent) => onDrop(e, null)}
                    folderId={null}
                    dropEnabled={dropEnabled}
                />
                {folders.map(folder => (
                    <SidebarItem
                        key={folder.id}
                        icon={Folder}
                        label={folder.name}
                        active={activeFolderId === folder.id}
                        onClick={() => setActiveFolderId(folder.id)}
                        onDrop={(e: React.DragEvent) => onDrop(e, folder.id)}
                        onDelete={() => onDelete(folder.id, folder.name)}
                        folderId={folder.id}
                        dropEnabled={dropEnabled}
                    />
                ))}
            </nav>

            {/* Sticky Create Folder section — always visible above the footer */}
            <div className="px-2 pb-2 border-b border-telegram-border">
                {showNewFolderInput ? (
                    <div className="px-3 py-2">
                        <input
                            autoFocus
                            type="text"
                            className="w-full bg-white/10 rounded px-2 py-1 text-sm text-white focus:outline-none focus:ring-1 focus:ring-telegram-primary"
                            placeholder="Folder Name"
                            value={newFolderName}
                            onChange={e => setNewFolderName(e.target.value)}
                            onKeyDown={e => e.key === 'Enter' && submitCreate()}
                            onBlur={() => !newFolderName && setShowNewFolderInput(false)}
                        />
                    </div>
                ) : (
                    <button
                        onClick={() => status === 'online' && setShowNewFolderInput(true)}
                        disabled={status !== 'online'}
                        title={status !== 'online' ? connectionStatusLabel(status) : undefined}
                        className="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium text-telegram-subtext hover:bg-telegram-hover hover:text-telegram-text transition-colors border border-dashed border-telegram-border disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                        <Plus className="w-4 h-4" />
                        Create Folder
                    </button>
                )}
            </div>

            <div className="p-4 border-t border-telegram-border">
                <div className="flex items-center gap-2 text-telegram-subtext text-xs">
                    <div className={`w-2 h-2 rounded-full ${statusDotClass}`}></div>
                    <span>{connectionStatusLabel(status)}</span>
                </div>

                <div className="flex gap-2 mt-4">
                    <button
                        onClick={onSync}
                        disabled={isSyncing || status !== 'online'}
                        className={`flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs font-medium text-blue-500 hover:text-blue-600 bg-blue-500/10 hover:bg-blue-500/20 rounded-lg transition-colors ${isSyncing || status !== 'online' ? 'opacity-50 cursor-not-allowed' : ''}`}
                        title={status !== 'online' ? 'Telegram session required to scan folders' : 'Scan for existing folders'}
                    >
                        <RefreshCw className={`w-3 h-3 ${isSyncing ? 'animate-spin' : ''}`} />
                        {isSyncing ? 'Syncing...' : 'Sync'}
                    </button>
                    <button
                        onClick={onLogout}
                        className="flex-1 flex items-center justify-center gap-2 px-3 py-2 text-xs font-medium text-red-500 hover:text-red-600 bg-red-500/10 hover:bg-red-500/20 rounded-lg transition-colors"
                        title="Sign Out"
                    >
                        <LogOut className="w-3 h-3" />
                        Logout
                    </button>
                </div>

                {bandwidth && <BandwidthWidget bandwidth={bandwidth} />}
            </div>

        </aside>
    )
}
