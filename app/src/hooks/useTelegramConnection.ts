import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Store } from '@tauri-apps/plugin-store';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { useConfirm } from '../context/ConfirmContext';
import { TelegramFolder } from '../types';
import { useNetworkStatus } from './useNetworkStatus';

import { isSessionLostError } from '../utils/sessionError';
import { canTransferFiles, classifyConnectionStatus, connectionStatusLabel, type ConnectionStatus } from '../types/connection';

export function useTelegramConnection(onLogoutParent: () => void) {
    const queryClient = useQueryClient();
    const { confirm } = useConfirm();

    const [folders, setFolders] = useState<TelegramFolder[]>([]);
    const [activeFolderId, setActiveFolderId] = useState<number | null>(null);
    const [store, setStore] = useState<Store | null>(null);
    const [isSyncing, setIsSyncing] = useState(false);
    const [isConnected, setIsConnected] = useState(false);
    const [connectionStatus, setConnectionStatus] = useState<ConnectionStatus>('checking');


    const networkIsOnline = useNetworkStatus();


    // Load persisted store and restore saved folders.
    // NOTE: The Telegram connection is already established by App.tsx before
    // Dashboard mounts, so we do NOT call cmd_connect here. This prevents
    // duplicate network runners and race conditions in the Rust backend.
    useEffect(() => {
        const initStore = async () => {
            try {
                let _store = await Store.load('config.json');
                const checkId = await _store.get<string>('api_id');
                if (!checkId) {
                    _store = await Store.load('settings.json');
                }
                setStore(_store);

                const savedFolders = await _store.get<TelegramFolder[]>('folders');
                if (savedFolders) setFolders(savedFolders);

                const savedActiveFolderId = await _store.get<number | null>('activeFolderId');
                if (savedActiveFolderId !== undefined) setActiveFolderId(savedActiveFolderId);

                // Files refresh after first cmd_check_connection (see refreshConnection effect)
            } catch {
                // store not available
            }
        };
        initStore();
    }, [queryClient]);


    useEffect(() => {
        let cancelled = false;
        const refreshConnection = async () => {
            if (!networkIsOnline) {
                if (!cancelled) {
                    setConnectionStatus('network_offline');
                    setIsConnected(false);
                }
                return;
            }
            try {
                const tgOk = await invoke<boolean>('cmd_check_connection');
                if (!cancelled) {
                    const status = classifyConnectionStatus(true, tgOk);
                    setConnectionStatus(status);
                    setIsConnected(tgOk);
                    if (tgOk) {
                        queryClient.invalidateQueries({ queryKey: ['files'] });
                    }
                }
            } catch {
                if (!cancelled) {
                    setConnectionStatus('session_lost');
                    setIsConnected(false);
                }
            }
        };
        refreshConnection();
        const interval = setInterval(refreshConnection, 30_000);
        return () => {
            cancelled = true;
            clearInterval(interval);
        };
    }, [networkIsOnline]);


    const isNetworkError = isSessionLostError;

    const forceLogout = useCallback(async () => {
        setIsConnected(false);
        setConnectionStatus('session_lost');
        try {
            await invoke('cmd_clean_cache').catch(() => { });
            if (store) {
                await store.delete('api_id');
                await store.delete('api_hash');
                await store.delete('folders');
                await store.save();
            }
        } catch {
            // best effort cleanup
        }
        toast.error("连接已断开，请重新登录");
        onLogoutParent();
    }, [onLogoutParent, store]);


    const handleLogout = async () => {
        if (!await confirm({ title: "Sign Out", message: "Are you sure you want to sign out? This will disconnect your active session.", confirmText: "Sign Out", variant: 'danger' })) return;

        try {
            await invoke('cmd_logout');
            await invoke('cmd_clean_cache');
            if (store) {
                await store.delete('api_id');
                await store.delete('api_hash');
                await store.delete('folders');
                await store.save();
            }
            onLogoutParent();
        } catch {
            toast.error("退出登录失败");
            onLogoutParent();
        }
    };

    const requireOnline = (): boolean => {
        if (!canTransferFiles(connectionStatus)) {
            toast.error(connectionStatusLabel(connectionStatus));
            return false;
        }
        return true;
    };

    const handleSyncFolders = async () => {
        if (!requireOnline()) return;
        if (!store) return;
        setIsSyncing(true);
        try {
            const foundFolders = await invoke<TelegramFolder[]>('cmd_scan_folders');
            const merged = [...folders];
            let added = 0;
            for (const f of foundFolders) {
                if (!merged.find(existing => existing.id === f.id)) {
                    merged.push(f);
                    added++;
                }
            }
            if (added > 0) {
                setFolders(merged);
                await store.set('folders', merged);
                await store.save();
            }

            const persisted =
                (await store.get<TelegramFolder[]>('folders')) ?? merged;
            const folderIds: (number | null)[] = [null, ...persisted.map(f => f.id)];
            const rebuilt = await invoke<{ folders_scanned: number; files_indexed: number }>(
                'cmd_rebuild_file_index',
                { folderIds },
            );
            queryClient.invalidateQueries({ queryKey: ['files'] });

            if (added > 0) {
                toast.success(
                    `扫描完成。新增 ${added} 个文件夹。已索引 ${rebuilt.files_indexed} 个文件，${rebuilt.folders_scanned} 个文件夹`,
                );
            } else {
                toast.success(
                    `同步完成。已索引 ${rebuilt.files_indexed} 个文件，${rebuilt.folders_scanned} 个文件夹`,
                );
            }
        } catch {
            toast.error("同步失败");
        } finally {
            setIsSyncing(false);
        }
    };

    const handleCreateFolder = async (name: string) => {
        if (!requireOnline()) return;
        if (!store) return;
        try {
            const newFolder = await invoke<TelegramFolder>('cmd_create_folder', { name });
            const updated = [...folders, newFolder];
            setFolders(updated);
            await store.set('folders', updated);
            await store.save();
            queryClient.invalidateQueries({ queryKey: ['files'] });
            toast.success(`文件夹 "${name}" 已创建`);
        } catch (e) {
            toast.error("创建文件夹失败: " + e);
            throw e;
        }
    };

    const handleFolderDelete = async (folderId: number, folderName: string) => {
        if (!requireOnline()) return;
        if (!await confirm({
            title: "Delete Folder",
            message: `Are you sure you want to delete "${folderName}"?\nThis will delete the channel on Telegram.`,
            confirmText: "Delete",
            variant: 'danger'
        })) return;

        try {
            await invoke('cmd_delete_folder', { folderId });
            const updated = folders.filter(f => f.id !== folderId);
            setFolders(updated);
            if (store) {
                await store.set('folders', updated);
                await store.save();
            }
            if (activeFolderId === folderId) setActiveFolderId(null);
            queryClient.invalidateQueries({ queryKey: ['files'] });
            toast.success(`文件夹 "${folderName}" 已删除`);
        } catch (e: unknown) {
            const errStr = String(e);
            if (errStr.includes("not found")) {
                if (await confirm({
                    title: "Folder Not Found",
                    message: `Folder "${folderName}" not found on Telegram (it may have been deleted externally).\nRemove from this app?`,
                    confirmText: "Remove",
                    variant: 'info'
                })) {
                    const updated = folders.filter(f => f.id !== folderId);
                    setFolders(updated);
                    if (store) {
                        await store.set('folders', updated);
                        await store.save();
                    }
                    if (activeFolderId === folderId) setActiveFolderId(null);
                }
            } else {
                toast.error(`删除文件夹失败: ${e}`);
            }
        }
    };


    const handleSetActiveFolderId = async (id: number | null) => {
        setActiveFolderId(id);
        if (store) {
            await store.set('activeFolderId', id);
            await store.save();
        }
    };

    return {
        store,
        folders,
        activeFolderId,
        setActiveFolderId: handleSetActiveFolderId,
        isSyncing,
        isConnected,
        connectionStatus,
        handleLogout,
        handleSyncFolders,
        handleCreateFolder,
        handleFolderDelete,
        isNetworkError,
        forceLogout
    };
}
