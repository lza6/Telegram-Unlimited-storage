import type { ReactNode } from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { toast } from 'sonner';
import { mockInvoke } from '../test-setup';
import { useFileOperations } from './useFileOperations';
import { createHookWrapper } from '../test-utils/hookWrapper';
import type { TelegramFile } from '../types';

const mockConfirm = vi.hoisted(() => vi.fn().mockResolvedValue(true));

vi.mock('../context/ConfirmContext', () => ({
    useConfirm: () => ({ confirm: mockConfirm }),
    ConfirmProvider: ({ children }: { children: ReactNode }) => children,
}));

const wrapper = createHookWrapper();

const sampleFiles: TelegramFile[] = [
    { id: 1, name: 'a.txt', folder_id: 10, size: 10, sizeStr: '10 B' },
    { id: 2, name: 'b.txt', folder_id: 10, size: 20, sizeStr: '20 B' },
];

describe('useFileOperations', () => {
    beforeEach(() => {
        mockInvoke.mockReset();
        mockInvoke.mockResolvedValue({
            moved: 1,
            oldMessageIds: [1],
            newMessageIds: [101],
            targetFolderId: 20,
        });
        mockConfirm.mockReset();
        mockConfirm.mockResolvedValue(true);
        vi.mocked(toast.error).mockClear();
        vi.mocked(toast.success).mockClear();
        vi.mocked(toast.info).mockClear();
    });

    it('blocks handleBulkMove when canBulkMove is false', async () => {
        const setSelectedIds = vi.fn();
        const { result } = renderHook(
            () =>
                useFileOperations(10, [1, 2], setSelectedIds, sampleFiles, undefined, undefined, {
                    canTransfer: () => true,
                    canBulkMove: () => false,
                    bulkMoveBlockedMessage: '需要 User 模式',
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleBulkMove(20);
        });

        expect(toast.error).toHaveBeenCalledWith('需要 User 模式');
        expect(mockInvoke).not.toHaveBeenCalledWith(
            'cmd_move_files',
            expect.anything(),
        );
    });

    it('blocks handleBulkMove when transfer guard fails first', async () => {
        const setSelectedIds = vi.fn();
        const { result } = renderHook(
            () =>
                useFileOperations(10, [1], setSelectedIds, sampleFiles, undefined, undefined, {
                    canTransfer: () => false,
                    transferBlockedMessage: '会话未就绪',
                    canBulkMove: () => false,
                    bulkMoveBlockedMessage: '需要 User 模式',
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleBulkMove(20);
        });

        expect(toast.error).toHaveBeenCalledWith('会话未就绪');
        expect(mockInvoke).not.toHaveBeenCalledWith(
            'cmd_move_files',
            expect.anything(),
        );
    });

    it('invokes cmd_move_files when bulk move is allowed', async () => {
        const setSelectedIds = vi.fn();
        const onFilesMoved = vi.fn();
        const { result } = renderHook(
            () =>
                useFileOperations(10, [1], setSelectedIds, sampleFiles, undefined, undefined, {
                    canTransfer: () => true,
                    canBulkMove: () => true,
                    onFilesMoved,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleBulkMove(20);
        });

        expect(mockInvoke).toHaveBeenCalledWith('cmd_move_files', {
            messageIds: [1],
            sourceFolderId: 10,
            targetFolderId: 20,
        });
        expect(toast.success).toHaveBeenCalledWith(expect.stringContaining('已移动'));
        expect(setSelectedIds).toHaveBeenCalledWith([]);
    });

    it('blocks handleBulkDownload when canTransfer is false', async () => {
        const queueBulkDownload = vi.fn();
        const setSelectedIds = vi.fn();
        const { result } = renderHook(
            () =>
                useFileOperations(10, [1], setSelectedIds, sampleFiles, queueBulkDownload, undefined, {
                    canTransfer: () => false,
                    transferBlockedMessage: '离线',
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleBulkDownload();
        });

        expect(toast.error).toHaveBeenCalledWith('离线');
        expect(queueBulkDownload).not.toHaveBeenCalled();
    });

    it('deletes file when confirmed and online', async () => {
        const setSelectedIds = vi.fn();
        const onFilesRemoved = vi.fn();
        mockInvoke.mockResolvedValue(undefined);
        const { result } = renderHook(
            () =>
                useFileOperations(10, [], setSelectedIds, sampleFiles, undefined, undefined, {
                    canTransfer: () => true,
                    onFilesRemoved,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleDelete(1);
        });

        expect(mockInvoke).toHaveBeenCalledWith('cmd_delete_file', {
            messageId: 1,
            folderId: 10,
        });
        expect(toast.success).toHaveBeenCalledWith('文件已删除');
        expect(onFilesRemoved).toHaveBeenCalledWith([1]);
    });

    it('skips delete when confirm returns false', async () => {
        mockConfirm.mockResolvedValueOnce(false);
        const { result } = renderHook(
            () =>
                useFileOperations(10, [], vi.fn(), sampleFiles, undefined, undefined, {
                    canTransfer: () => true,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleDelete(1);
        });

        expect(mockInvoke).not.toHaveBeenCalledWith('cmd_delete_file', expect.anything());
    });

    it('bulk deletes selected files and reports partial failure', async () => {
        const setSelectedIds = vi.fn();
        let deleteCalls = 0;
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'cmd_delete_file') {
                deleteCalls += 1;
                if (deleteCalls === 2) return Promise.reject(new Error('fail'));
            }
            return Promise.resolve(undefined);
        });
        const { result } = renderHook(
            () =>
                useFileOperations(10, [1, 2], setSelectedIds, sampleFiles, undefined, undefined, {
                    canTransfer: () => true,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleBulkDelete();
        });

        expect(toast.success).toHaveBeenCalledWith(expect.stringContaining('已删除 1'));
        expect(toast.error).toHaveBeenCalledWith(expect.stringContaining('失败'));
        expect(setSelectedIds).toHaveBeenCalledWith([2]);
    });

    it('handleDownloadFolder shows info when folder empty', async () => {
        const { result } = renderHook(
            () =>
                useFileOperations(10, [], vi.fn(), [], vi.fn(), undefined, {
                    canTransfer: () => true,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleDownloadFolder();
        });

        expect(toast.info).toHaveBeenCalledWith('文件夹为空');
    });

    it('handleBulkDownload queues files and clears selection', async () => {
        const queueBulkDownload = vi.fn().mockResolvedValue(undefined);
        const setSelectedIds = vi.fn();
        const { result } = renderHook(
            () =>
                useFileOperations(10, [1, 2], setSelectedIds, sampleFiles, queueBulkDownload, undefined, {
                    canTransfer: () => true,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleBulkDownload();
        });

        expect(queueBulkDownload).toHaveBeenCalledWith(
            expect.arrayContaining([
                expect.objectContaining({ id: 1 }),
                expect.objectContaining({ id: 2 }),
            ]),
            10,
        );
        expect(setSelectedIds).toHaveBeenCalledWith([]);
    });

    it('handleDownloadFolder queues all displayed files', async () => {
        const queueBulkDownload = vi.fn().mockResolvedValue(undefined);
        const { result } = renderHook(
            () =>
                useFileOperations(10, [], vi.fn(), sampleFiles, queueBulkDownload, undefined, {
                    canTransfer: () => true,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleDownloadFolder();
        });

        expect(queueBulkDownload).toHaveBeenCalledWith(sampleFiles, 10);
    });

    it('shows error when download queue unavailable', async () => {
        const { result } = renderHook(
            () =>
                useFileOperations(10, [1], vi.fn(), sampleFiles, undefined, undefined, {
                    canTransfer: () => true,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleBulkDownload();
        });

        expect(toast.error).toHaveBeenCalledWith('下载队列不可用');
    });

    it('reports move error via toast', async () => {
        mockInvoke.mockRejectedValueOnce(new Error('network down'));
        const { result } = renderHook(
            () =>
                useFileOperations(10, [1], vi.fn(), sampleFiles, undefined, undefined, {
                    canTransfer: () => true,
                    canBulkMove: () => true,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleBulkMove(20);
        });

        expect(toast.error).toHaveBeenCalledWith(expect.stringContaining('Move failed'));
    });

    it('shows info when bulk move moves zero files', async () => {
        mockInvoke.mockResolvedValueOnce({
            moved: 0,
            oldMessageIds: [],
            newMessageIds: [],
            targetFolderId: 10,
        });
        const { result } = renderHook(
            () =>
                useFileOperations(10, [1], vi.fn(), sampleFiles, undefined, undefined, {
                    canTransfer: () => true,
                    canBulkMove: () => true,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleBulkMove(10);
        });

        expect(toast.info).toHaveBeenCalledWith('所选文件已在目标文件夹中');
    });

    it('partial bulk move keeps failed selection and reports partial error', async () => {
        const setSelectedIds = vi.fn();
        mockInvoke
            .mockResolvedValueOnce({
                moved: 1,
                oldMessageIds: [1],
                newMessageIds: [101],
                targetFolderId: 20,
            })
            .mockRejectedValueOnce(new Error('group two failed'));
        const multiFolderFiles: TelegramFile[] = [
            { id: 1, name: 'a.txt', folder_id: 10, size: 10, sizeStr: '10 B' },
            { id: 2, name: 'b.txt', folder_id: 11, size: 20, sizeStr: '20 B' },
        ];
        const { result } = renderHook(
            () =>
                useFileOperations(10, [1, 2], setSelectedIds, multiFolderFiles, undefined, undefined, {
                    canTransfer: () => true,
                    canBulkMove: () => true,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleBulkMove(20);
        });

        expect(toast.success).toHaveBeenCalledWith('已移动 1 个文件');
        expect(toast.error).toHaveBeenCalledWith(expect.stringContaining('部分移动失败'));
        expect(setSelectedIds).toHaveBeenCalledWith([2]);
    });

    it('does not call onSuccess when bulk move partially fails', async () => {
        const onSuccess = vi.fn();
        mockInvoke
            .mockResolvedValueOnce({
                moved: 1,
                oldMessageIds: [1],
                newMessageIds: [101],
                targetFolderId: 20,
            })
            .mockRejectedValueOnce(new Error('group two failed'));
        const multiFolderFiles: TelegramFile[] = [
            { id: 1, name: 'a.txt', folder_id: 10, size: 10, sizeStr: '10 B' },
            { id: 2, name: 'b.txt', folder_id: 11, size: 20, sizeStr: '20 B' },
        ];
        const { result } = renderHook(
            () =>
                useFileOperations(10, [1, 2], vi.fn(), multiFolderFiles, undefined, undefined, {
                    canTransfer: () => true,
                    canBulkMove: () => true,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleBulkMove(20, onSuccess);
        });

        expect(onSuccess).not.toHaveBeenCalled();
    });

    it('bulk delete invokes onSessionError when last failure is session lost', async () => {
        const onSessionError = vi.fn();
        mockInvoke.mockRejectedValue(new Error('Session expired — sign in again'));
        const { result } = renderHook(
            () =>
                useFileOperations(10, [1], vi.fn(), sampleFiles, undefined, onSessionError, {
                    canTransfer: () => true,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleBulkDelete();
        });

        expect(onSessionError).toHaveBeenCalledWith(expect.stringContaining('Session expired'));
    });

    it('handleDownloadFolder shows error when queue unavailable', async () => {
        const { result } = renderHook(
            () =>
                useFileOperations(10, [], vi.fn(), sampleFiles, undefined, undefined, {
                    canTransfer: () => true,
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleDownloadFolder();
        });

        expect(toast.error).toHaveBeenCalledWith('下载队列不可用');
    });
});
