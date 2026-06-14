import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { toast } from 'sonner';
import { mockInvoke, mockListen, mockDialogOpen, mockDialogSave, mockStoreInstance, resetStoreData } from '../test-setup';
import { useFileDownload } from './useFileDownload';
import { createHookWrapper } from '../test-utils/hookWrapper';
const wrapper = createHookWrapper();

describe('useFileDownload', () => {
    beforeEach(() => {
        resetStoreData();
        mockInvoke.mockReset();
        mockInvoke.mockResolvedValue(undefined);
        mockListen.mockReset();
        mockListen.mockImplementation(() => Promise.resolve(vi.fn()));
        mockDialogOpen.mockReset();
        mockDialogOpen.mockResolvedValue(null);
        mockDialogSave.mockReset();
        mockDialogSave.mockResolvedValue('C:\\Downloads\\photo.png');
        vi.mocked(toast.success).mockClear();
        vi.mocked(toast.error).mockClear();
        vi.mocked(toast.info).mockClear();
    });

    it('queueDownload enqueues item with correct metadata', async () => {
        mockInvoke.mockImplementation(
            () => new Promise(() => {
                /* keep in downloading for assertion window */
            }),
        );

        const { result } = renderHook(() => useFileDownload(null), { wrapper });

        act(() => {
            result.current.queueDownload(42, 'doc.pdf', null);
        });

        await waitFor(() => {
            expect(result.current.downloadQueue).toHaveLength(1);
        });
        expect(result.current.downloadQueue[0]?.messageId).toBe(42);
        expect(result.current.downloadQueue[0]?.filename).toBe('doc.pdf');
        expect(result.current.downloadQueue[0]?.status).not.toBe('error');
    });

    it('blocks queueDownload when canTransfer is false', () => {
        const { result } = renderHook(
            () =>
                useFileDownload(null, {
                    canTransfer: () => false,
                    transferBlockedMessage: 'Not connected',
                }),
            { wrapper },
        );

        act(() => {
            result.current.queueDownload(1, 'a.txt', null);
        });

        expect(result.current.downloadQueue).toHaveLength(0);
        expect(toast.error).toHaveBeenCalledWith('Not connected');
    });

    it('allows queueDownload when canDownload true without canTransfer', () => {
        mockInvoke.mockImplementation(
            () => new Promise(() => {
                /* keep in downloading */
            }),
        );

        const { result } = renderHook(
            () =>
                useFileDownload(null, {
                    canTransfer: () => false,
                    canDownload: () => true,
                }),
            { wrapper },
        );

        act(() => {
            result.current.queueDownload(99, 'bot-mode.bin', 3);
        });

        expect(result.current.downloadQueue).toHaveLength(1);
        expect(result.current.downloadQueue[0]?.messageId).toBe(99);
    });

    it('processes pending download to success', async () => {
        const { result } = renderHook(() => useFileDownload(null), { wrapper });

        act(() => {
            result.current.queueDownload(100, 'photo.png', 5);
        });

        await waitFor(() => {
            expect(result.current.downloadQueue[0]?.status).toBe('success');
        });
        expect(mockInvoke).toHaveBeenCalledWith('cmd_download_file', expect.objectContaining({
            messageId: expect.any(Number),
            savePath: 'C:\\Downloads\\photo.png',
            transferId: expect.any(String),
        }));
        expect(toast.success).toHaveBeenCalled();
    });

    it('removes item when save dialog is cancelled', async () => {
        mockDialogSave.mockResolvedValueOnce(null);

        const { result } = renderHook(() => useFileDownload(null), { wrapper });
        act(() => {
            result.current.queueDownload(200, 'video.mp4', null);
        });

        await waitFor(() => {
            expect(result.current.downloadQueue).toHaveLength(0);
        });
    });

    it('marks cancelled when invoke returns Transfer cancelled', async () => {
        mockInvoke.mockRejectedValueOnce(new Error('Transfer cancelled by user'));

        const { result } = renderHook(() => useFileDownload(null), { wrapper });
        act(() => {
            result.current.queueDownload(300, 'x.zip', null);
        });

        await waitFor(() => {
            expect(result.current.downloadQueue[0]?.status).toBe('cancelled');
        });
        expect(toast.error).not.toHaveBeenCalled();
    });

    it('calls onSessionError on session-lost download failure', async () => {
        const onSessionError = vi.fn();
        mockInvoke.mockRejectedValueOnce(new Error('session expired — please log in'));

        const { result } = renderHook(
            () => useFileDownload(null, { onSessionError }),
            { wrapper },
        );
        act(() => {
            result.current.queueDownload(400, 'secret.pdf', null);
        });

        await waitFor(() => {
            expect(onSessionError).toHaveBeenCalled();
        });
        expect(result.current.downloadQueue[0]?.status).toBe('error');
        expect(toast.error).toHaveBeenCalledWith(expect.stringContaining('secret.pdf'));
    });

    it('clearFinished removes success and keeps error items', async () => {
        let downloadCalls = 0;
        mockInvoke.mockImplementation((cmd) => {
            if (cmd === 'cmd_download_file') {
                downloadCalls += 1;
                if (downloadCalls === 2) {
                    return Promise.reject(new Error('disk full'));
                }
            }
            return Promise.resolve(undefined);
        });

        const { result } = renderHook(() => useFileDownload(null), { wrapper });
        act(() => {
            result.current.queueDownload(1, 'ok.bin', null);
            result.current.queueDownload(2, 'bad.bin', null);
        });

        await waitFor(() => {
            const statuses = result.current.downloadQueue.map((i) => i.status);
            expect(statuses).toContain('success');
            expect(statuses).toContain('error');
        });

        act(() => {
            result.current.clearFinished();
        });

        expect(result.current.downloadQueue).toHaveLength(1);
        expect(result.current.downloadQueue[0]?.filename).toBe('bad.bin');
        expect(result.current.downloadQueue[0]?.status).toBe('error');
    });

    it('clearFinished empties queue when only success items', async () => {
        const { result } = renderHook(() => useFileDownload(null), { wrapper });

        act(() => {
            result.current.queueDownload(1, 'ok.bin', null);
        });
        await waitFor(() => expect(result.current.downloadQueue[0]?.status).toBe('success'));

        act(() => {
            result.current.clearFinished();
        });
        expect(result.current.downloadQueue).toHaveLength(0);
    });

    it('retryItem resets error to pending', async () => {
        mockInvoke.mockRejectedValueOnce(new Error('network timeout'));

        const { result } = renderHook(() => useFileDownload(null), { wrapper });
        act(() => {
            result.current.queueDownload(500, 'retry.me', null);
        });
        await waitFor(() => expect(result.current.downloadQueue[0]?.status).toBe('error'));

        mockInvoke.mockResolvedValue(undefined);
        act(() => {
            result.current.retryItem(result.current.downloadQueue[0]!.id);
        });

        await waitFor(() => {
            expect(result.current.downloadQueue[0]?.status).toBe('success');
        });
    });

    it('updates progress on download-progress event', async () => {
        let progressHandler: ((event: { payload: Record<string, unknown> }) => void) | undefined;
        mockListen.mockImplementation((event, handler) => {
            if (event === 'download-progress') {
                progressHandler = handler as typeof progressHandler;
            }
            return Promise.resolve(vi.fn());
        });

        const { result } = renderHook(() => useFileDownload(null), { wrapper });

        act(() => {
            result.current.queueDownload(600, 'big.bin', null);
        });

        await waitFor(() => expect(progressHandler).toBeDefined());

        const id = result.current.downloadQueue[0]?.id;
        act(() => {
            progressHandler?.({
                payload: {
                    id,
                    percent: 77,
                    uploaded_bytes: 770,
                    total_bytes: 1000,
                    speed_bytes_per_sec: 50,
                },
            });
        });

        await waitFor(() => {
            expect(result.current.downloadQueue[0]?.progress).toBe(77);
        });
    });

    it('queueBulkDownload queues files after directory pick', async () => {
        mockDialogOpen.mockResolvedValueOnce('C:\\Bulk');
        mockInvoke.mockImplementation(() => new Promise(() => undefined));

        const { result } = renderHook(() => useFileDownload(null), { wrapper });

        await act(async () => {
            await result.current.queueBulkDownload(
                [
                    { id: 1, name: 'a.png', size: 1, sizeStr: '1B' },
                    { id: 2, name: 'b.pdf', size: 2, sizeStr: '2B' },
                ],
                9,
            );
        });

        await waitFor(() => {
            expect(result.current.downloadQueue).toHaveLength(2);
        });
        expect(toast.info).toHaveBeenCalledWith('已加入 2 个下载任务');
    });

    it('cancelAll cancels downloading items', async () => {
        mockInvoke.mockImplementation(() => new Promise(() => undefined));

        const { result } = renderHook(() => useFileDownload(null), { wrapper });
        act(() => {
            result.current.queueDownload(1, 'live.bin', null);
        });

        await waitFor(() => {
            expect(result.current.downloadQueue[0]?.status).toBe('downloading');
        });

        act(() => {
            result.current.cancelAll();
        });

        expect(mockInvoke).toHaveBeenCalledWith('cmd_cancel_transfer', expect.any(Object));
        expect(result.current.downloadQueue[0]?.status).toBe('cancelled');
        expect(toast.info).toHaveBeenCalledWith('所有下载已取消');
    });

    it('cancelItem on downloading invokes cmd_cancel_transfer', async () => {
        mockInvoke.mockImplementation(() => new Promise(() => undefined));

        const { result } = renderHook(() => useFileDownload(null), { wrapper });
        act(() => {
            result.current.queueDownload(99, 'live.dat', null);
        });

        await waitFor(() => {
            expect(result.current.downloadQueue[0]?.status).toBe('downloading');
        });

        const id = result.current.downloadQueue[0]!.id;
        act(() => {
            result.current.cancelItem(id);
        });

        expect(mockInvoke).toHaveBeenCalledWith('cmd_cancel_transfer', { transferId: id });
        expect(result.current.downloadQueue[0]?.status).toBe('cancelled');
    });

    it('restores pending downloads from store on init', async () => {
        resetStoreData({
            downloadQueue: [
                {
                    id: 'stored-dl',
                    messageId: 50,
                    filename: 'restored.bin',
                    folderId: null,
                    status: 'pending',
                    savePath: 'C:\\Downloads\\restored.bin',
                },
            ],
        });
        mockInvoke.mockImplementation(() => new Promise(() => undefined));

        const { result } = renderHook(
            () => useFileDownload(mockStoreInstance as never),
            { wrapper },
        );

        await waitFor(() => {
            expect(result.current.downloadQueue).toHaveLength(1);
        });
        expect(result.current.downloadQueue[0]?.filename).toBe('restored.bin');
        expect(toast.info).toHaveBeenCalledWith('已恢复 1 个待下载任务');
    });

    it('restores pending downloads after hook remount', async () => {
        resetStoreData({
            downloadQueue: [
                {
                    id: 'stored-remount-dl',
                    messageId: 51,
                    filename: 'remount.bin',
                    folderId: null,
                    status: 'pending',
                    savePath: 'C:\\Downloads\\remount.bin',
                },
            ],
        });

        const blockedOpts = { canTransfer: () => false, transferBlockedMessage: 'offline' };

        const first = renderHook(
            () => useFileDownload(mockStoreInstance as never, blockedOpts),
            { wrapper },
        );
        await waitFor(() => {
            expect(first.result.current.downloadQueue).toHaveLength(1);
        });
        first.unmount();

        vi.mocked(toast.info).mockClear();
        const { result } = renderHook(
            () => useFileDownload(mockStoreInstance as never, blockedOpts),
            { wrapper },
        );
        await waitFor(() => {
            expect(result.current.downloadQueue).toHaveLength(1);
        });
        expect(result.current.downloadQueue[0]?.id).toBe('stored-remount-dl');
        expect(toast.info).toHaveBeenCalledWith('已恢复 1 个待下载任务');
    });
});
