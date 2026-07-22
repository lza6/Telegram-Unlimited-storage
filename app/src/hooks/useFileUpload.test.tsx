import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { toast } from 'sonner';
import { mockInvoke, mockListen, mockDialogOpen, mockStoreInstance, resetStoreData } from '../test-setup';
import { useFileUpload } from './useFileUpload';
import { createHookWrapper } from '../test-utils/hookWrapper';
import type { QueueItem } from '../types';

const wrapper = createHookWrapper();

function pendingItem(overrides: Partial<QueueItem> = {}): QueueItem {
    return {
        id: 'upload-1',
        path: 'C:\\videos\\clip.mp4',
        folderId: null,
        status: 'pending',
        ...overrides,
    };
}

describe('useFileUpload', () => {
    beforeEach(() => {
        resetStoreData();
        mockInvoke.mockReset();
        mockInvoke.mockResolvedValue(undefined);
        mockListen.mockReset();
        mockListen.mockImplementation(() => Promise.resolve(vi.fn()));
        mockDialogOpen.mockReset();
        mockDialogOpen.mockResolvedValue(null);
        vi.mocked(toast.error).mockClear();
        vi.mocked(toast.info).mockClear();
    });

    it('processes pending item to success via invoke', async () => {
        const { result } = renderHook(() => useFileUpload(null, null), { wrapper });

        act(() => {
            result.current.setUploadQueue([pendingItem()]);
        });

        await waitFor(() => {
            expect(result.current.uploadQueue[0]?.status).toBe('success');
        });
        expect(mockInvoke).toHaveBeenCalledWith('cmd_upload_file', {
            path: 'C:\\videos\\clip.mp4',
            folderId: null,
            transferId: 'upload-1',
        });
    });

    it('classifies FILE_TOO_BIG and shows dedicated toast', async () => {
        mockInvoke.mockRejectedValueOnce(new Error('FILE_TOO_BIG limit exceeded'));

        const { result } = renderHook(() => useFileUpload(null, null), { wrapper });
        act(() => {
            result.current.setUploadQueue([pendingItem({ id: 'big-1' })]);
        });

        await waitFor(() => {
            expect(result.current.uploadQueue[0]?.status).toBe('error');
        });
        expect(toast.error).toHaveBeenCalledWith(
            '上传失败：Telegram 限制单文件 2GB，建议拆分大文件夹',
        );
    });

    it('calls onSessionError for session-lost failures', async () => {
        const onSessionError = vi.fn();
        mockInvoke.mockRejectedValueOnce(new Error('Telegram client is not connected'));

        const { result } = renderHook(
            () => useFileUpload(null, null, { onSessionError }),
            { wrapper },
        );
        act(() => {
            result.current.setUploadQueue([pendingItem({ id: 'sess-1' })]);
        });

        await waitFor(() => {
            expect(onSessionError).toHaveBeenCalled();
        });
    });

    it('blocks retry when canTransfer is false', async () => {
        const { result } = renderHook(
            () =>
                useFileUpload(null, null, {
                    canTransfer: () => false,
                    transferBlockedMessage: 'Session not ready',
                }),
            { wrapper },
        );

        act(() => {
            result.current.setUploadQueue([
                pendingItem({ id: 'retry-1', status: 'error', error: 'fail' }),
            ]);
        });

        act(() => {
            result.current.retryItem('retry-1');
        });

        expect(result.current.uploadQueue[0]?.status).toBe('error');
        expect(toast.error).toHaveBeenCalledWith('Session not ready');
    });

    it('retryItem clears error and re-queues failed upload', async () => {
        mockInvoke.mockImplementation(
            () => new Promise(() => {
                /* stall upload so we can observe re-queue */
            }),
        );

        const { result } = renderHook(() => useFileUpload(null, null), { wrapper });

        act(() => {
            result.current.setUploadQueue([
                pendingItem({ id: 'retry-ok', status: 'error', error: 'temporary' }),
            ]);
        });

        act(() => {
            result.current.retryItem('retry-ok');
        });

        await waitFor(() => {
            expect(result.current.uploadQueue[0]?.error).toBeUndefined();
            expect(result.current.uploadQueue[0]?.status).toBe('uploading');
        });
    });

    it('clearFinished removes only success items', () => {
        const { result } = renderHook(() => useFileUpload(null, null), { wrapper });

        act(() => {
            result.current.setUploadQueue([
                pendingItem({ id: 'a', status: 'success' }),
                pendingItem({ id: 'b', status: 'error', error: 'x' }),
            ]);
        });

        act(() => {
            result.current.clearFinished();
        });

        expect(result.current.uploadQueue).toHaveLength(1);
        expect(result.current.uploadQueue[0]?.id).toBe('b');
    });

    it('cancelItem on uploading invokes cmd_cancel_transfer', async () => {
        mockInvoke.mockImplementation(
            () => new Promise(() => {
                /* never resolves — stays uploading */
            }),
        );

        const { result } = renderHook(() => useFileUpload(null, null), { wrapper });
        act(() => {
            result.current.setUploadQueue([pendingItem({ id: 'cancel-me' })]);
        });

        await waitFor(() => {
            expect(result.current.uploadQueue[0]?.status).toBe('uploading');
        });

        act(() => {
            result.current.cancelItem('cancel-me');
        });

        expect(mockInvoke).toHaveBeenCalledWith('cmd_cancel_transfer', { transferId: 'cancel-me' });
        expect(result.current.uploadQueue[0]?.status).toBe('cancelled');
    });

    it('updates progress on upload-progress event', async () => {
        let progressHandler: ((event: { payload: Record<string, unknown> }) => void) | undefined;
        mockListen.mockImplementation((event, handler) => {
            if (event === 'upload-progress') {
                progressHandler = handler as typeof progressHandler;
            }
            return Promise.resolve(vi.fn());
        });

        const { result } = renderHook(() => useFileUpload(null, null), { wrapper });
        act(() => {
            result.current.setUploadQueue([
                pendingItem({ id: 'prog-1', status: 'uploading', progress: 0 }),
            ]);
        });

        await waitFor(() => expect(progressHandler).toBeDefined());

        act(() => {
            progressHandler?.({
                payload: {
                    id: 'prog-1',
                    percent: 42,
                    uploaded_bytes: 420,
                    total_bytes: 1000,
                    speed_bytes_per_sec: 100,
                },
            });
        });

        await waitFor(() => {
            expect(result.current.uploadQueue[0]?.progress).toBe(42);
            expect(result.current.uploadQueue[0]?.uploadedBytes).toBe(420);
        });
    });

    it('does not process pending when canTransfer is false', async () => {
        const { result } = renderHook(
            () => useFileUpload(null, null, { canTransfer: () => false }),
            { wrapper },
        );

        act(() => {
            result.current.setUploadQueue([pendingItem({ id: 'blocked' })]);
        });

        await new Promise((r) => setTimeout(r, 50));
        expect(result.current.uploadQueue[0]?.status).toBe('pending');
        expect(mockInvoke).not.toHaveBeenCalledWith('cmd_upload_file', expect.anything());
    });

    it('enqueueUploadPaths queues paths and respects guardTransfer', async () => {
        const { result } = renderHook(
            () =>
                useFileUpload(3, null, {
                    canTransfer: () => false,
                    transferBlockedMessage: 'Blocked',
                }),
            { wrapper },
        );

        act(() => {
            result.current.enqueueUploadPaths(['C:\\a.txt']);
        });

        expect(result.current.uploadQueue).toHaveLength(0);
        expect(toast.error).toHaveBeenCalled();

        const { result: ok } = renderHook(() => useFileUpload(3, null), { wrapper });
        act(() => {
            ok.current.enqueueUploadPaths(['C:\\a.txt', 'C:\\b.txt']);
        });
        expect(ok.current.uploadQueue).toHaveLength(2);
        expect(toast.info).toHaveBeenCalledWith('已加入 2 个上传任务');
    });

    it('handleManualUpload queues files from dialog selection', async () => {
        mockDialogOpen.mockResolvedValueOnce(['C:\\a.txt', 'C:\\b.txt']);

        const { result } = renderHook(() => useFileUpload(7, null), { wrapper });

        await act(async () => {
            await result.current.handleManualUpload();
        });

        expect(result.current.uploadQueue).toHaveLength(2);
        expect(result.current.uploadQueue.every((i) => i.folderId === 7)).toBe(true);
        expect(toast.info).toHaveBeenCalledWith('已加入 2 个上传任务');
    });

    it('handleManualUpload respects guardTransfer', async () => {
        const { result } = renderHook(
            () =>
                useFileUpload(null, null, {
                    canTransfer: () => false,
                    transferBlockedMessage: 'Blocked',
                }),
            { wrapper },
        );

        await act(async () => {
            await result.current.handleManualUpload();
        });

        expect(mockDialogOpen).not.toHaveBeenCalled();
        expect(result.current.uploadQueue).toHaveLength(0);
    });

    it('handleFolderUpload zips folder and queues zip path', async () => {
        mockDialogOpen.mockResolvedValueOnce('C:\\Projects\\demo');
        mockInvoke.mockImplementation((cmd) => {
            if (cmd === 'cmd_zip_folder') {
                return Promise.resolve('C:\\Temp\\demo.zip');
            }
            return new Promise(() => undefined);
        });

        const { result } = renderHook(() => useFileUpload(null, null), { wrapper });

        await act(async () => {
            await result.current.handleFolderUpload();
        });

        await waitFor(() => {
            expect(result.current.uploadQueue).toHaveLength(1);
        });
        expect(result.current.uploadQueue[0]?.path).toBe('C:\\Temp\\demo.zip');
        expect(result.current.uploadQueue[0]?.tempZipPath).toBe('C:\\Temp\\demo.zip');
    });

    it('cancelAll cancels uploading items and invokes cmd_cancel_transfer', async () => {
        mockInvoke.mockImplementation(
            () => new Promise(() => undefined),
        );

        const { result } = renderHook(() => useFileUpload(null, null), { wrapper });
        act(() => {
            result.current.setUploadQueue([
                pendingItem({ id: 'up-a', status: 'uploading' }),
                pendingItem({ id: 'up-b', status: 'success' }),
            ]);
        });

        act(() => {
            result.current.cancelAll();
        });

        expect(mockInvoke).toHaveBeenCalledWith('cmd_cancel_transfer', { transferId: 'up-a' });
        expect(result.current.uploadQueue.find((i) => i.id === 'up-a')?.status).toBe('cancelled');
        expect(toast.info).toHaveBeenCalledWith('所有上传已取消');
    });

    it('cancelItem removes pending item without invoke', () => {
        const { result } = renderHook(
            () => useFileUpload(null, null, { canTransfer: () => false }),
            { wrapper },
        );
        act(() => {
            result.current.setUploadQueue([pendingItem({ id: 'pend-rm' })]);
        });

        act(() => {
            result.current.cancelItem('pend-rm');
        });

        expect(result.current.uploadQueue).toHaveLength(0);
        expect(mockInvoke).not.toHaveBeenCalledWith('cmd_cancel_transfer', expect.anything());
    });

    it('restores pending uploads from store on init', async () => {
        resetStoreData({
            uploadQueue: [pendingItem({ id: 'stored-1', status: 'pending' })],
        });
        mockInvoke.mockImplementation(() => new Promise(() => undefined));

        const { result } = renderHook(
            () => useFileUpload(null, mockStoreInstance as never),
            { wrapper },
        );

        await waitFor(() => {
            expect(result.current.uploadQueue).toHaveLength(1);
        });
        expect(toast.info).toHaveBeenCalledWith('已恢复 1 个待上传任务');
    });

    it('restores pending uploads after hook remount', async () => {
        resetStoreData({
            uploadQueue: [pendingItem({ id: 'stored-remount', status: 'pending' })],
        });

        const blockedOpts = { canTransfer: () => false, transferBlockedMessage: 'offline' };

        const first = renderHook(
            () => useFileUpload(null, mockStoreInstance as never, blockedOpts),
            { wrapper },
        );
        await waitFor(() => {
            expect(first.result.current.uploadQueue).toHaveLength(1);
        });
        first.unmount();

        vi.mocked(toast.info).mockClear();
        const { result } = renderHook(
            () => useFileUpload(null, mockStoreInstance as never, blockedOpts),
            { wrapper },
        );
        await waitFor(() => {
            expect(result.current.uploadQueue).toHaveLength(1);
        });
        expect(result.current.uploadQueue[0]?.id).toBe('stored-remount');
        expect(toast.info).toHaveBeenCalledWith('已恢复 1 个待上传任务');
    });

    it('handleFolderUpload shows error when zip fails', async () => {
        mockDialogOpen.mockResolvedValueOnce('C:\\folder');
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'cmd_zip_folder') return Promise.reject(new Error('zip failed'));
            return Promise.resolve(undefined);
        });

        const { result } = renderHook(() => useFileUpload(null, null), { wrapper });

        await act(async () => {
            await result.current.handleFolderUpload();
        });

        expect(toast.error).toHaveBeenCalledWith('压缩文件夹失败: Error: zip failed');
        expect(result.current.uploadQueue).toHaveLength(0);
    });
});
