import '@testing-library/jest-dom';
import { vi } from 'vitest';

const mocks = vi.hoisted(() => {
    const mockListenUnlisten = vi.fn();
    const mockListen = vi.fn().mockImplementation(() => Promise.resolve(mockListenUnlisten));
    const mockInvoke = vi.fn().mockResolvedValue(undefined);
    const mockDialogOpen = vi.fn().mockResolvedValue(null);
    const mockDialogSave = vi.fn().mockResolvedValue(null);
    const storeData = new Map<string, unknown>();

    function wireStoreMocks() {
        mockStoreInstance.get.mockImplementation(async (key: string) =>
            storeData.has(key) ? storeData.get(key) : null,
        );
        mockStoreInstance.set.mockImplementation(async (key: string, value: unknown) => {
            storeData.set(key, value);
        });
        mockStoreInstance.delete.mockImplementation(async (key: string) => {
            storeData.delete(key);
        });
        mockStoreInstance.save.mockResolvedValue(undefined);
    }

    const mockStoreInstance = {
        get: vi.fn(),
        set: vi.fn(),
        save: vi.fn(),
        delete: vi.fn(),
    };
    wireStoreMocks();

    function resetStoreData(defaults?: Record<string, unknown>) {
        storeData.clear();
        if (defaults) {
            for (const [key, value] of Object.entries(defaults)) {
                storeData.set(key, value);
            }
        }
        mockStoreInstance.get.mockReset();
        mockStoreInstance.set.mockReset();
        mockStoreInstance.save.mockReset();
        mockStoreInstance.delete.mockReset();
        wireStoreMocks();
    }

    return {
        mockListenUnlisten,
        mockListen,
        mockInvoke,
        mockDialogOpen,
        mockDialogSave,
        mockStoreInstance,
        resetStoreData,
    };
});

export const {
    mockInvoke,
    mockListen,
    mockListenUnlisten,
    mockDialogOpen,
    mockDialogSave,
    mockStoreInstance,
    resetStoreData,
} = mocks;

vi.mock('@tauri-apps/api/core', () => ({
    invoke: mocks.mockInvoke,
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: mocks.mockListen,
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
    open: mocks.mockDialogOpen,
    save: mocks.mockDialogSave,
}));

vi.mock('@tauri-apps/plugin-store', () => ({
    load: vi.fn().mockResolvedValue(mocks.mockStoreInstance),
    Store: {
        load: vi.fn().mockResolvedValue(mocks.mockStoreInstance),
    },
}));

vi.mock('sonner', () => ({
    toast: {
        success: vi.fn(),
        error: vi.fn(),
        info: vi.fn(),
    },
    Toaster: () => null,
}));
