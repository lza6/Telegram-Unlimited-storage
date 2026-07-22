import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { QueueItem } from '../types';
import { useSettings } from '../context/SettingsContext';
import type { Store } from '@tauri-apps/plugin-store';
import { isSessionLostError } from '../utils/sessionError';
import { buildUploadQueueEntries, classifyUploadFailure } from '../lib/uploadPure';
import {
    applyCancelAllTransfers,
    applyCancelTransferItem,
    applyRetryTransferItem,
    computeAvailableSlots,
    filterClearFinishedTransfers,
    selectPendingTransfers,
} from '../lib/queuePure';
import { useTransferProgress } from './useTransferProgress';

export function useFileUpload(
    activeFolderId: number | null,
    store: Store | null,
    opts?: {
        onSessionError?: (message: string) => void;
        canTransfer?: () => boolean;
        transferBlockedMessage?: string;
    },
) {
    const queryClient = useQueryClient();
    const { settings } = useSettings();
    const [uploadQueue, setUploadQueue] = useState<QueueItem[]>([]);
    const [initialized, setInitialized] = useState(false);
    const cancelledRef = useRef<Set<string>>(new Set());
    const inFlightRef = useRef<Set<string>>(new Set());

    const maxConcurrent = Math.max(1, Math.min(10, settings.maxConcurrentUploads || 1));

    // Use shared progress listener
    useTransferProgress('upload-progress', (payload) => {
        setUploadQueue(q => q.map(i =>
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
        store.get<QueueItem[]>('uploadQueue').then((saved) => {
            if (saved && saved.length > 0) {
                const pending = saved.filter(i => i.status === 'pending');
                if (pending.length > 0) {
                    setUploadQueue(pending);
                    toast.info(`已恢复 ${pending.length} 个待上传任务`);
                }
            }
            setInitialized(true);
        });
    }, [store, initialized]);

    useEffect(() => {
        if (!store || !initialized) return;
        const pending = uploadQueue.filter(i => i.status === 'pending');
        store.set('uploadQueue', pending).then(() => store.save());
    }, [store, uploadQueue, initialized]);

    const cleanupTempZip = async (item: QueueItem) => {
        if (item.tempZipPath) {
            try {
                await invoke('cmd_delete_temp_zip', { path: item.tempZipPath });
            } catch {
                // Best-effort cleanup
            }
        }
    };

    const processItem = useCallback(async (item: QueueItem) => {
        if (inFlightRef.current.has(item.id)) return;
        if (opts?.canTransfer && !opts.canTransfer()) return;
        inFlightRef.current.add(item.id);
        setUploadQueue(q => q.map(i => i.id === item.id ? { ...i, status: 'uploading', progress: 0 } : i));
        try {
            await invoke('cmd_upload_file', { path: item.path, folderId: item.folderId, transferId: item.id });
            if (cancelledRef.current.has(item.id)) {
                cancelledRef.current.delete(item.id);
            } else {
                setUploadQueue(q => q.map(i => i.id === item.id ? { ...i, status: 'success', progress: 100 } : i));
                queryClient.invalidateQueries({ queryKey: ['files'] });
            }
            await cleanupTempZip(item);
        } catch (e) {
            if (!cancelledRef.current.has(item.id)) {
                const errMsg = String(e);
                const kind = classifyUploadFailure(errMsg, { isSessionLost: isSessionLostError });
                if (kind === 'cancelled') {
                    setUploadQueue(q => q.map(i => i.id === item.id ? { ...i, status: 'cancelled' } : i));
                } else if (kind === 'file_too_big') {
                    setUploadQueue(q => q.map(i => i.id === item.id ? { ...i, status: 'error', error: errMsg } : i));
                    toast.error('上传失败：Telegram 限制单文件 2GB，建议拆分大文件夹');
                } else {
                    setUploadQueue(q => q.map(i => i.id === item.id ? { ...i, status: 'error', error: errMsg } : i));
                    toast.error(`${item.path.split(/[/\\]/).pop()} 上传失败: ${e}`);
                    if (kind === 'session_lost' && opts?.onSessionError) {
                        opts.onSessionError(errMsg);
                    }
                }
            } else {
                cancelledRef.current.delete(item.id);
            }
            await cleanupTempZip(item);
        } finally {
            inFlightRef.current.delete(item.id);
            setUploadQueue(q => [...q]);
        }
    }, [queryClient, opts]);

    useEffect(() => {
        if (opts?.canTransfer && !opts.canTransfer()) return;
        const slots = computeAvailableSlots(inFlightRef.current.size, maxConcurrent);
        const pending = selectPendingTransfers(uploadQueue, inFlightRef.current, slots);
        pending.forEach(item => {
            void processItem(item);
        });
    }, [uploadQueue, maxConcurrent, processItem, opts]);

    const guardTransfer = (): boolean => {
        if (opts?.canTransfer && !opts.canTransfer()) {
            toast.error(opts.transferBlockedMessage || 'Telegram 会话未就绪');
            return false;
        }
        return true;
    };

    const enqueueUploadPaths = useCallback((paths: string[]) => {
        if (!guardTransfer()) return;
        const filePaths = paths.filter((p) => p && !p.endsWith('/') && !p.endsWith('\\'));
        if (filePaths.length === 0) {
            toast.error('未检测到可上传的文件（文件夹请用 Upload Folder）');
            return;
        }
        const newItems: QueueItem[] = buildUploadQueueEntries(filePaths, activeFolderId).map((entry) => ({
            ...entry,
            id: Math.random().toString(36).substr(2, 9),
        }));
        setUploadQueue(prev => [...prev, ...newItems]);
        toast.info(`已加入 ${filePaths.length} 个上传任务`);
    }, [activeFolderId, opts?.canTransfer, opts?.transferBlockedMessage]);

    const handleManualUpload = async () => {
        if (!guardTransfer()) return;
        try {
            const selected = await open({ multiple: true, directory: false });
            if (selected) {
                const paths = Array.isArray(selected) ? selected : [selected];
                enqueueUploadPaths(paths);
            }
        } catch {
            toast.error("打开文件对话框失败");
        }
    };

    const handleFolderUpload = async () => {
        if (!guardTransfer()) return;
        try {
            const selected = await open({ multiple: false, directory: true, title: 'Select Folder to Upload' });
            if (!selected) return;

            const folderPath = Array.isArray(selected) ? selected[0] : selected;
            if (!folderPath) return;

            const folderName = folderPath.split(/[/\\]/).pop() || 'folder';

            if (settings.zipFolders) {
                toast.info(`正在压缩 "${folderName}"...`);
                try {
                    const zipPath = await invoke<string>('cmd_zip_folder', { folderPath });
                    const item: QueueItem = {
                        id: Math.random().toString(36).substr(2, 9),
                        path: zipPath,
                        folderId: activeFolderId,
                        status: 'pending',
                        tempZipPath: zipPath,
                    };
                    setUploadQueue(prev => [...prev, item]);
                    toast.success(`"${folderName}.zip" 已加入上传队列`);
                } catch (e) {
                    toast.error(`压缩文件夹失败: ${e}`);
                }
            } else {
                toast.info('不支持不压缩直接上传文件夹，请在设置中启用"上传前压缩文件夹"');
            }
        } catch {
            toast.error("打开文件夹对话框失败");
        }
    };

    const cancelAll = () => {
        setUploadQueue(q => {
            const { queue, invokeCancelIds } = applyCancelAllTransfers(q, 'uploading');
            invokeCancelIds.forEach(id => {
                cancelledRef.current.add(id);
                invoke('cmd_cancel_transfer', { transferId: id }).catch(() => {});
            });
            return queue;
        });
        toast.info('所有上传已取消');
    };

    const cancelItem = (id: string) => {
        setUploadQueue(q => {
            const { queue, invokeCancelId } = applyCancelTransferItem(q, id, 'uploading');
            if (invokeCancelId) {
                cancelledRef.current.add(invokeCancelId);
                invoke('cmd_cancel_transfer', { transferId: invokeCancelId }).catch(() => {});
            }
            return queue;
        });
    };

    const retryItem = (id: string) => {
        if (!guardTransfer()) return;
        setUploadQueue(q => applyRetryTransferItem(q, id));
    };

    const clearFinished = () => {
        setUploadQueue(q => filterClearFinishedTransfers(q));
    };

    return {
        uploadQueue,
        setUploadQueue,
        enqueueUploadPaths,
        handleManualUpload,
        handleFolderUpload,
        cancelAll,
        cancelItem,
        retryItem,
        clearFinished,
    };
}
