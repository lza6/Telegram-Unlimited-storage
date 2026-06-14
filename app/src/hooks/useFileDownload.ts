import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { save, open } from '@tauri-apps/plugin-dialog';
import { toast } from 'sonner';
import { DownloadItem, TelegramFile } from '../types';
import { useSettings } from '../context/SettingsContext';
import type { Store } from '@tauri-apps/plugin-store';
import { isSessionLostError } from '../utils/sessionError';
import { buildBulkDownloadItems, classifyDownloadFailure } from '../lib/downloadPure';
import {
    applyCancelAllTransfers,
    applyCancelTransferItem,
    applyRetryTransferItem,
    computeAvailableSlots,
    filterClearFinishedTransfers,
    selectPendingTransfers,
} from '../lib/queuePure';
import { useTransferProgress } from './useTransferProgress';

export function useFileDownload(
    store: Store | null,
    opts?: {
        onSessionError?: (message: string) => void;
        canTransfer?: () => boolean;
        canDownload?: () => boolean;
        transferBlockedMessage?: string;
        downloadBlockedMessage?: string;
    },
) {
    const { settings } = useSettings();
    const [downloadQueue, setDownloadQueue] = useState<DownloadItem[]>([]);
    const [initialized, setInitialized] = useState(false);
    const cancelledRef = useRef<Set<string>>(new Set());
    const inFlightRef = useRef<Set<string>>(new Set());

    const maxConcurrent = Math.max(1, Math.min(10, settings.maxConcurrentDownloads || 1));

    // Use shared progress listener
    useTransferProgress('download-progress', (payload) => {
        setDownloadQueue(q => q.map(i =>
            i.id === payload.id ? {
                ...i,
                progress: payload.percent,
                uploadedBytes: payload.uploaded_bytes,
                totalBytes: payload.total_bytes,
                speedBytesPerSec: payload.speed_bytes_per_sec,
            } : i
        ));
    });

    useEffect(() => {
        if (!store || initialized) return;
        store.get<DownloadItem[]>('downloadQueue').then((saved) => {
            if (saved && saved.length > 0) {
                const pending = saved.filter(i => i.status === 'pending');
                if (pending.length > 0) {
                    setDownloadQueue(pending);
                    toast.info(`已恢复 ${pending.length} 个待下载任务`);
                }
            }
            setInitialized(true);
        });
    }, [store, initialized]);

    useEffect(() => {
        if (!store || !initialized) return;
        const pending = downloadQueue.filter(i => i.status === 'pending');
        store.set('downloadQueue', pending).then(() => store.save());
    }, [store, downloadQueue, initialized]);

    const isDownloadAllowed = useCallback((): boolean => {
        if (opts?.canDownload) return opts.canDownload();
        if (opts?.canTransfer) return opts.canTransfer();
        return true;
    }, [opts]);

    const processItem = useCallback(async (item: DownloadItem) => {
        if (inFlightRef.current.has(item.id)) return;
        if (!isDownloadAllowed()) return;
        inFlightRef.current.add(item.id);
        setDownloadQueue(q => q.map(i => i.id === item.id ? { ...i, status: 'downloading', progress: 0 } : i));

        try {
            const savePath = item.savePath || await save({ defaultPath: item.filename });
            if (!savePath) {
                setDownloadQueue(q => q.filter(i => i.id !== item.id));
                return;
            }

            await invoke('cmd_download_file', {
                messageId: item.messageId,
                savePath,
                folderId: item.folderId,
                transferId: item.id
            });

            if (cancelledRef.current.has(item.id)) {
                cancelledRef.current.delete(item.id);
            } else {
                setDownloadQueue(q => q.map(i => i.id === item.id ? { ...i, status: 'success', progress: 100 } : i));
                toast.success(`下载完成: ${item.filename}`);
            }
        } catch (e) {
            if (!cancelledRef.current.has(item.id)) {
                const errMsg = String(e);
                const kind = classifyDownloadFailure(errMsg, { isSessionLost: isSessionLostError });
                if (kind === 'cancelled') {
                    setDownloadQueue(q => q.map(i => i.id === item.id ? { ...i, status: 'cancelled' } : i));
                } else {
                    setDownloadQueue(q => q.map(i => i.id === item.id ? { ...i, status: 'error', error: errMsg } : i));
                    toast.error(`下载失败: ${item.filename}`);
                    if (kind === 'session_lost' && opts?.onSessionError) {
                        opts.onSessionError(errMsg);
                    }
                }
            } else {
                cancelledRef.current.delete(item.id);
            }
        } finally {
            inFlightRef.current.delete(item.id);
            setDownloadQueue(q => [...q]);
        }
    }, [isDownloadAllowed, opts]);

    useEffect(() => {
        if (!isDownloadAllowed()) return;
        const slots = computeAvailableSlots(inFlightRef.current.size, maxConcurrent);
        const pending = selectPendingTransfers(downloadQueue, inFlightRef.current, slots);
        pending.forEach(item => {
            void processItem(item);
        });
    }, [downloadQueue, maxConcurrent, isDownloadAllowed, processItem]);

    const guardDownload = (): boolean => {
        if (isDownloadAllowed()) return true;
        toast.error(
            opts?.downloadBlockedMessage
                || opts?.transferBlockedMessage
                || '下载不可用 — Telegram 会话或 Bot 服务未就绪',
        );
        return false;
    };

    const queueDownload = (messageId: number, filename: string, folderId: number | null) => {
        if (!guardDownload()) return;
        const newItem: DownloadItem = {
            id: Math.random().toString(36).substr(2, 9),
            messageId,
            filename,
            folderId,
            status: 'pending'
        };
        setDownloadQueue(prev => [...prev, newItem]);
    };

    const queueBulkDownload = async (files: TelegramFile[], folderId: number | null) => {
        if (!guardDownload()) return;
        const dirPath = await open({
            directory: true,
            multiple: false,
            title: "Select Download Destination"
        });
        if (!dirPath) return;

        const newItems: DownloadItem[] = buildBulkDownloadItems(files, dirPath, folderId).map((entry) => ({
            ...entry,
            id: Math.random().toString(36).substr(2, 9),
            status: 'pending' as const,
        }));

        setDownloadQueue(prev => [...prev, ...newItems]);
        toast.info(`已加入 ${files.length} 个下载任务`);
    };

    const clearFinished = () => {
        setDownloadQueue(q => filterClearFinishedTransfers(q));
    };

    const cancelAll = () => {
        setDownloadQueue(q => {
            const { queue, invokeCancelIds } = applyCancelAllTransfers(q, 'downloading');
            invokeCancelIds.forEach(id => {
                cancelledRef.current.add(id);
                invoke('cmd_cancel_transfer', { transferId: id }).catch(() => {});
            });
            return queue;
        });
        toast.info('所有下载已取消');
    };

    const cancelItem = (id: string) => {
        setDownloadQueue(q => {
            const { queue, invokeCancelId } = applyCancelTransferItem(q, id, 'downloading');
            if (invokeCancelId) {
                cancelledRef.current.add(invokeCancelId);
                invoke('cmd_cancel_transfer', { transferId: invokeCancelId }).catch(() => {});
            }
            return queue;
        });
    };

    const retryItem = (id: string) => {
        if (!guardDownload()) return;
        setDownloadQueue(q => applyRetryTransferItem(q, id));
    };

    return {
        downloadQueue,
        queueDownload,
        queueBulkDownload,
        clearFinished,
        cancelAll,
        cancelItem,
        retryItem,
    };
}
