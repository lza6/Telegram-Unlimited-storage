import { useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { useConfirm } from '../context/ConfirmContext';
import { MoveFilesPayload, TelegramFile } from '../types';
import { isSessionLostError } from '../utils/sessionError';
import { executeMoveGroups } from '../lib/moveExecution';
import {
    resolveFileFolderId,
    planMoveGroups,
    pruneSelectedIdsAfterDelete,
} from '../utils';

export function useFileOperations(
    activeFolderId: number | null,
    selectedIds: number[],
    setSelectedIds: (ids: number[]) => void,
    displayedFiles: TelegramFile[],
    queueBulkDownload?: (files: TelegramFile[], folderId: number | null) => Promise<void>,
    onSessionError?: (message: string) => void,
    opts?: {
        canTransfer?: () => boolean;
        transferBlockedMessage?: string;
        canDownload?: () => boolean;
        downloadBlockedMessage?: string;
        canIndexDelete?: () => boolean;
        indexDeleteBlockedMessage?: string;
        canBulkMove?: () => boolean;
        bulkMoveBlockedMessage?: string;
        onFilesRemoved?: (ids: number[]) => void;
        onFilesMoved?: (payload: MoveFilesPayload) => void;
    },
) {
    const queryClient = useQueryClient();
    const { confirm } = useConfirm();

    const guardTransfer = useCallback((): boolean => {
        if (opts?.canTransfer && !opts.canTransfer()) {
            toast.error(opts.transferBlockedMessage || 'Telegram 会话未就绪');
            return false;
        }
        return true;
    }, [opts]);

    const guardDownload = useCallback((): boolean => {
        if (opts?.canDownload && opts.canDownload()) return true;
        if (opts?.canTransfer && opts.canTransfer()) return true;
        toast.error(
            opts?.downloadBlockedMessage
                || opts?.transferBlockedMessage
                || '下载不可用 — Telegram 会话或 Bot 服务未就绪',
        );
        return false;
    }, [opts]);

    const guardDelete = useCallback((): boolean => {
        if (opts?.canTransfer && opts.canTransfer()) return true;
        if (opts?.canIndexDelete && opts.canIndexDelete()) return true;
        toast.error(
            opts?.indexDeleteBlockedMessage
                || opts?.transferBlockedMessage
                || '无法删除 — Telegram 会话或 Bot 服务未就绪',
        );
        return false;
    }, [opts]);

    const guardBulkMove = useCallback((): boolean => {
        if (!guardTransfer()) return false;
        if (opts?.canBulkMove && !opts.canBulkMove()) {
            toast.error(
                opts.bulkMoveBlockedMessage || '当前传输模式不支持批量移动',
            );
            return false;
        }
        return true;
    }, [guardTransfer, opts]);

    const reportError = useCallback((action: string, e: unknown) => {
        const errMsg = String(e);
        toast.error(`${action} failed: ${errMsg}`);
        if (onSessionError && isSessionLostError(errMsg)) {
            onSessionError(errMsg);
        }
    }, [onSessionError]);

    const folderForId = useCallback((id: number): number | null => {
        const file = displayedFiles.find((f) => f.id === id);
        return resolveFileFolderId(file ?? {}, activeFolderId);
    }, [displayedFiles, activeFolderId]);

    const handleDelete = useCallback(async (id: number) => {
        if (!guardDelete()) return;
        if (!await confirm({ title: "Delete File", message: "Are you sure you want to delete this file?", confirmText: "Delete", variant: 'danger' })) return;

        try {
            await invoke('cmd_delete_file', { messageId: id, folderId: folderForId(id) });
            await invoke('cmd_delete_image_thumbnail', { messageId: id }).catch(() => {});
            queryClient.invalidateQueries({ queryKey: ['files'] });
            toast.success("文件已删除");
            opts?.onFilesRemoved?.([id]);
        } catch (e) {
            reportError('Delete', e);
        }
    }, [guardDelete, confirm, folderForId, queryClient, opts, reportError]);

    const handleBulkDelete = useCallback(async () => {
        if (!guardDelete()) return;
        if (selectedIds.length === 0) return;
        if (!await confirm({ title: "Delete Files", message: `Are you sure you want to delete ${selectedIds.length} files?`, confirmText: "Delete All", variant: 'danger' })) return;

        let success = 0;
        let fail = 0;
        let lastError = '';
        const removedIds: number[] = [];

        for (const id of selectedIds) {
            try {
                await invoke('cmd_delete_file', { messageId: id, folderId: folderForId(id) });
                await invoke('cmd_delete_image_thumbnail', { messageId: id }).catch(() => {});
                success++;
                removedIds.push(id);
            } catch (e) {
                fail++;
                lastError = String(e);
            }
        }

        setSelectedIds(pruneSelectedIdsAfterDelete(selectedIds, removedIds));
        queryClient.invalidateQueries({ queryKey: ['files'] });

        if (removedIds.length > 0) {
            opts?.onFilesRemoved?.(removedIds);
        }
        if (success > 0) toast.success(`已删除 ${success} 个文件`);
        if (fail > 0) {
            toast.error(`删除 ${fail} 个文件失败`);
            if (onSessionError && isSessionLostError(lastError)) {
                onSessionError(lastError);
            }
        }
    }, [guardDelete, selectedIds, confirm, folderForId, setSelectedIds, queryClient, opts, onSessionError]);

    const handleBulkDownload = useCallback(async () => {
        if (!guardDownload()) return;
        if (selectedIds.length === 0) return;

        const targetFiles = displayedFiles.filter((f) => selectedIds.includes(f.id));
        if (queueBulkDownload) {
            await queueBulkDownload(targetFiles, activeFolderId);
            setSelectedIds([]);
            return;
        }
        toast.error('下载队列不可用');
    }, [guardDownload, selectedIds, displayedFiles, queueBulkDownload, activeFolderId, setSelectedIds]);

    const handleBulkMove = useCallback(async (targetFolderId: number | null, onSuccess?: () => void) => {
        if (!guardBulkMove()) return;
        if (selectedIds.length === 0) return;

        const groups = planMoveGroups(selectedIds, displayedFiles, activeFolderId, targetFolderId);
        const { moved, movedOldIds, mergedPayload, failures } = await executeMoveGroups(
            groups,
            targetFolderId,
        );

        if (movedOldIds.length > 0) {
            setSelectedIds(pruneSelectedIdsAfterDelete(selectedIds, movedOldIds));
            if (mergedPayload) opts?.onFilesMoved?.(mergedPayload);
        }

        if (moved > 0) toast.success(`已移动 ${moved} 个文件`);
        if (failures.length > 0) {
            const detail = failures.length === groups.length
                ? failures[0]
                : `部分移动失败（${failures.length}/${groups.length}）：${failures[0]}`;
            toast.error(`Move failed: ${detail}`);
            const sessionErr = failures.find((f) => isSessionLostError(f));
            if (onSessionError && sessionErr) {
                onSessionError(sessionErr);
            }
        } else if (moved === 0) {
            toast.info('所选文件已在目标文件夹中');
        }

        queryClient.invalidateQueries({ queryKey: ['files'] });
        if (onSuccess && moved > 0 && failures.length === 0) onSuccess();
    }, [guardBulkMove, selectedIds, displayedFiles, activeFolderId, queryClient, setSelectedIds, opts, onSessionError]);

    const handleDownloadFolder = useCallback(async () => {
        if (!guardDownload()) return;
        if (displayedFiles.length === 0) {
            toast.info("文件夹为空");
            return;
        }
        if (queueBulkDownload) {
            await queueBulkDownload(displayedFiles, activeFolderId);
            return;
        }
        toast.error('下载队列不可用');
    }, [guardDownload, displayedFiles, queueBulkDownload, activeFolderId]);

    return {
        handleDelete,
        handleBulkDelete,
        handleBulkDownload,
        handleBulkMove,
        handleDownloadFolder,
    };
}
