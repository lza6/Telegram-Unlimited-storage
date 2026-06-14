import type { ReactNode } from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { toast } from 'sonner';
import { useTelegramConnection } from './useTelegramConnection';
import { createHookWrapper } from '../test-utils/hookWrapper';
import { mockInvoke, mockStoreInstance, resetStoreData } from '../test-setup';
import { connectionStatusLabel } from '../types/connection';

const networkState = vi.hoisted(() => ({ online: true }));
const mockConfirm = vi.hoisted(() => vi.fn().mockResolvedValue(true));

vi.mock('./useNetworkStatus', () => ({
    useNetworkStatus: () => networkState.online,
}));

vi.mock('../context/ConfirmContext', () => ({
    useConfirm: () => ({ confirm: mockConfirm }),
    ConfirmProvider: ({ children }: { children: ReactNode }) => children,
}));

const wrapper = createHookWrapper();

describe('useTelegramConnection', () => {
    beforeEach(() => {
        networkState.online = true;
        resetStoreData({
            api_id: '12345',
            folders: [{ id: 10, name: 'Saved' }],
            activeFolderId: 10,
        });
        mockInvoke.mockReset();
        mockInvoke.mockResolvedValue(true);
        mockConfirm.mockReset();
        mockConfirm.mockResolvedValue(true);
        vi.mocked(toast.error).mockClear();
        vi.mocked(toast.success).mockClear();
    });

    it('transitions to online when network up and Telegram probe succeeds', async () => {
        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });

        await waitFor(() => {
            expect(result.current.connectionStatus).toBe('online');
        });
        expect(result.current.isConnected).toBe(true);
        expect(mockInvoke).toHaveBeenCalledWith('cmd_check_connection');
    });

    it('sets session_lost when cmd_check_connection throws', async () => {
        mockInvoke.mockRejectedValue(new Error('not connected'));

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });

        await waitFor(() => {
            expect(result.current.connectionStatus).toBe('session_lost');
        });
        expect(result.current.isConnected).toBe(false);
    });

    it('sets network_offline without probing Telegram when network is down', async () => {
        networkState.online = false;

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });

        await waitFor(() => {
            expect(result.current.connectionStatus).toBe('network_offline');
        });
        expect(mockInvoke).not.toHaveBeenCalledWith('cmd_check_connection');
    });

    it('restores folders and activeFolderId from store on init', async () => {
        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });

        await waitFor(() => {
            expect(result.current.folders).toEqual([{ id: 10, name: 'Saved' }]);
            expect(result.current.activeFolderId).toBe(10);
        });
    });

    it('blocks handleSyncFolders when not online and shows status toast', async () => {
        mockInvoke.mockRejectedValue(new Error('session lost'));

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });

        await waitFor(() => {
            expect(result.current.connectionStatus).toBe('session_lost');
        });

        await act(async () => {
            await result.current.handleSyncFolders();
        });

        expect(mockInvoke).not.toHaveBeenCalledWith('cmd_scan_folders');
        expect(toast.error).toHaveBeenCalledWith(
            connectionStatusLabel('session_lost'),
        );
    });

    it('forceLogout cleans cache and notifies parent', async () => {
        const onLogout = vi.fn();
        const { result } = renderHook(() => useTelegramConnection(onLogout), { wrapper });

        await waitFor(() => expect(result.current.store).not.toBeNull());

        await act(async () => {
            await result.current.forceLogout();
        });

        expect(mockInvoke).toHaveBeenCalledWith('cmd_clean_cache');
        expect(mockStoreInstance.delete).toHaveBeenCalledWith('api_id');
        expect(onLogout).toHaveBeenCalled();
        expect(toast.error).toHaveBeenCalledWith('连接已断开，请重新登录');
    });

    it('handleLogout skips when confirm returns false', async () => {
        mockConfirm.mockResolvedValueOnce(false);
        const onLogout = vi.fn();

        const { result } = renderHook(() => useTelegramConnection(onLogout), { wrapper });
        await waitFor(() => expect(result.current.connectionStatus).toBe('online'));

        await act(async () => {
            await result.current.handleLogout();
        });

        expect(mockInvoke).not.toHaveBeenCalledWith('cmd_logout');
        expect(onLogout).not.toHaveBeenCalled();
    });

    it('handleLogout signs out when confirm returns true', async () => {
        const onLogout = vi.fn();

        const { result } = renderHook(() => useTelegramConnection(onLogout), { wrapper });
        await waitFor(() => expect(result.current.connectionStatus).toBe('online'));

        await act(async () => {
            await result.current.handleLogout();
        });

        expect(mockInvoke).toHaveBeenCalledWith('cmd_logout');
        expect(mockInvoke).toHaveBeenCalledWith('cmd_clean_cache');
        expect(onLogout).toHaveBeenCalled();
    });

    it('handleSetActiveFolderId persists activeFolderId to store', async () => {
        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });
        await waitFor(() => expect(result.current.store).not.toBeNull());

        await act(async () => {
            await result.current.setActiveFolderId(99);
        });

        expect(result.current.activeFolderId).toBe(99);
        expect(mockStoreInstance.set).toHaveBeenCalledWith('activeFolderId', 99);
        expect(mockStoreInstance.save).toHaveBeenCalled();
    });

    it('handleCreateFolder creates folder and persists to store', async () => {
        const newFolder = { id: 99, name: 'New Folder' };
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'cmd_create_folder') return Promise.resolve(newFolder);
            return Promise.resolve(true);
        });

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });
        await waitFor(() => expect(result.current.connectionStatus).toBe('online'));

        await act(async () => {
            await result.current.handleCreateFolder('New Folder');
        });

        expect(mockInvoke).toHaveBeenCalledWith('cmd_create_folder', { name: 'New Folder' });
        expect(result.current.folders).toContainEqual(newFolder);
        expect(mockStoreInstance.set).toHaveBeenCalledWith('folders', expect.arrayContaining([newFolder]));
        expect(toast.success).toHaveBeenCalledWith('文件夹 "New Folder" 已创建');
    });

    it('handleCreateFolder shows error toast on failure', async () => {
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'cmd_create_folder') return Promise.reject(new Error('Telegram error'));
            return Promise.resolve(true);
        });

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });
        await waitFor(() => expect(result.current.connectionStatus).toBe('online'));

        await act(async () => {
            try {
                await result.current.handleCreateFolder('Test');
            } catch {
                // Expected to throw
            }
        });

        expect(toast.error).toHaveBeenCalledWith('创建文件夹失败: Error: Telegram error');
    });

    it('handleCreateFolder blocks when not online', async () => {
        mockInvoke.mockRejectedValue(new Error('session lost'));

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });
        await waitFor(() => expect(result.current.connectionStatus).toBe('session_lost'));

        await act(async () => {
            await result.current.handleCreateFolder('Test');
        });

        expect(mockInvoke).not.toHaveBeenCalledWith('cmd_create_folder', expect.anything());
        expect(toast.error).toHaveBeenCalledWith(connectionStatusLabel('session_lost'));
    });

    it('handleFolderDelete deletes folder and updates activeFolderId if needed', async () => {
        resetStoreData({
            api_id: '12345',
            folders: [
                { id: 10, name: 'Saved' },
                { id: 20, name: 'ToDelete' },
            ],
            activeFolderId: 20,
        });

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });
        await waitFor(() => expect(result.current.folders.length).toBe(2));

        await act(async () => {
            await result.current.handleFolderDelete(20, 'ToDelete');
        });

        expect(mockInvoke).toHaveBeenCalledWith('cmd_delete_folder', { folderId: 20 });
        expect(result.current.folders).toHaveLength(1);
        expect(result.current.folders.find(f => f.id === 20)).toBeUndefined();
        expect(result.current.activeFolderId).toBeNull();
        expect(toast.success).toHaveBeenCalledWith('文件夹 "ToDelete" 已删除');
    });

    it('handleFolderDelete shows error toast on failure', async () => {
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'cmd_delete_folder') return Promise.reject(new Error('Cannot delete'));
            return Promise.resolve(true);
        });
        mockConfirm.mockResolvedValue(true);

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });
        await waitFor(() => expect(result.current.connectionStatus).toBe('online'));

        await act(async () => {
            await result.current.handleFolderDelete(99, 'TestFolder');
        });

        expect(toast.error).toHaveBeenCalledWith('删除文件夹失败: Error: Cannot delete');
    });

    it('handleFolderDelete handles not found case with confirm', async () => {
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'cmd_delete_folder') return Promise.reject(new Error('not found'));
            return Promise.resolve(true);
        });
        mockConfirm.mockResolvedValueOnce(true).mockResolvedValueOnce(true);

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });
        await waitFor(() => expect(result.current.connectionStatus).toBe('online'));

        await act(async () => {
            await result.current.handleFolderDelete(99, 'MissingFolder');
        });

        // Should show the "Folder Not Found" confirm dialog
        expect(mockConfirm).toHaveBeenCalledWith(expect.objectContaining({
            title: 'Folder Not Found'
        }));
    });

    it('handleFolderDelete blocks when not online', async () => {
        mockInvoke.mockRejectedValue(new Error('session lost'));

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });
        await waitFor(() => expect(result.current.connectionStatus).toBe('session_lost'));

        await act(async () => {
            await result.current.handleFolderDelete(99, 'Test');
        });

        expect(mockInvoke).not.toHaveBeenCalledWith('cmd_delete_folder', expect.anything());
    });

    it('requireOnline returns false and shows toast when not connected', async () => {
        mockInvoke.mockRejectedValue(new Error('session lost'));

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });
        await waitFor(() => expect(result.current.connectionStatus).toBe('session_lost'));

        // The requireOnline is internal, tested via handleSyncFolders
        await act(async () => {
            await result.current.handleSyncFolders();
        });

        expect(mockInvoke).not.toHaveBeenCalledWith('cmd_scan_folders');
        expect(toast.error).toHaveBeenCalled();
    });

    it('initStore falls back to settings.json when config.json has no api_id', async () => {
        resetStoreData();

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });
        await waitFor(() => expect(result.current.store).not.toBeNull());
    });

    it('handleSyncFolders merges new folders and rebuilds index', async () => {
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'cmd_check_connection') return Promise.resolve(true);
            if (cmd === 'cmd_scan_folders') {
                return Promise.resolve([{ id: 11, name: 'Discovered' }]);
            }
            if (cmd === 'cmd_rebuild_file_index') {
                return Promise.resolve({ folders_scanned: 2, files_indexed: 10 });
            }
            return Promise.resolve(true);
        });

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });
        await waitFor(() => expect(result.current.connectionStatus).toBe('online'));

        await act(async () => {
            await result.current.handleSyncFolders();
        });

        expect(mockInvoke).toHaveBeenCalledWith('cmd_scan_folders');
        expect(mockInvoke).toHaveBeenCalledWith('cmd_rebuild_file_index', {
            folderIds: [null, 10, 11],
        });
        expect(result.current.folders).toEqual([
            { id: 10, name: 'Saved' },
            { id: 11, name: 'Discovered' },
        ]);
        expect(toast.success).toHaveBeenCalledWith(
            expect.stringContaining('新增 1 个文件夹'),
        );
    });

    it('handleSyncFolders shows sync toast when no new folders found', async () => {
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'cmd_check_connection') return Promise.resolve(true);
            if (cmd === 'cmd_scan_folders') return Promise.resolve([{ id: 10, name: 'Saved' }]);
            if (cmd === 'cmd_rebuild_file_index') {
                return Promise.resolve({ folders_scanned: 1, files_indexed: 3 });
            }
            return Promise.resolve(true);
        });

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });
        await waitFor(() => expect(result.current.connectionStatus).toBe('online'));

        await act(async () => {
            await result.current.handleSyncFolders();
        });

        expect(toast.success).toHaveBeenCalledWith(
            expect.stringContaining('同步完成'),
        );
        expect(toast.success).not.toHaveBeenCalledWith(
            expect.stringContaining('新增'),
        );
    });

    it('sets session_lost when Telegram probe returns false', async () => {
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'cmd_check_connection') return Promise.resolve(false);
            return Promise.resolve(true);
        });

        const { result } = renderHook(() => useTelegramConnection(vi.fn()), { wrapper });

        await waitFor(() => {
            expect(result.current.connectionStatus).toBe('session_lost');
        });
        expect(result.current.isConnected).toBe(false);
    });

    it(
        're-probes cmd_check_connection every 30s while network is online',
        async () => {
            vi.useFakeTimers();
            try {
                mockInvoke.mockImplementation((cmd: string) => {
                    if (cmd === 'cmd_check_connection') return Promise.resolve(true);
                    return Promise.resolve(true);
                });

                renderHook(() => useTelegramConnection(vi.fn()), { wrapper });

                await act(async () => {
                    await vi.runOnlyPendingTimersAsync();
                });

                const probeCalls = () =>
                    mockInvoke.mock.calls.filter((c) => c[0] === 'cmd_check_connection')
                        .length;
                const initial = probeCalls();
                expect(initial).toBeGreaterThanOrEqual(1);

                await act(async () => {
                    await vi.advanceTimersByTimeAsync(30_000);
                });

                expect(probeCalls()).toBeGreaterThan(initial);
            } finally {
                vi.useRealTimers();
            }
        },
        10_000,
    );
});
