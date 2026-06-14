import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ShareDialog } from './ShareDialog';

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

vi.mock('../../context/SettingsContext', () => ({
    useSettings: () => ({
        settings: { globalDomain: '' },
        updateSetting: vi.fn(),
    }),
}));

import { invoke } from '@tauri-apps/api/core';

const baseFile = {
    id: 42,
    name: 'photo.png',
    size: 1024,
    sizeStr: '1 KB',
    type: 'file' as const,
    folder_id: 7,
};

describe('ShareDialog', () => {
    beforeEach(() => {
        vi.mocked(invoke).mockReset();
    });

    it('allows generate when shareReady without User session', async () => {
        vi.mocked(invoke).mockResolvedValueOnce({
            id: 'tok',
            file_name: 'photo.png',
            file_size: 1024,
            created_at: 1,
            expires_at: null,
            has_password: false,
            link: 'http://127.0.0.1:14201/d/tok',
        });

        render(
            <ShareDialog
                file={baseFile}
                activeFolderId={7}
                shareReady={true}
                onClose={vi.fn()}
            />,
        );

        fireEvent.click(screen.getByRole('button', { name: /Generate Shareable Link/i }));

        await waitFor(() => {
            expect(invoke).toHaveBeenCalledWith('cmd_create_share', expect.objectContaining({
                messageId: 42,
                fileName: 'photo.png',
            }));
        });
        expect(await screen.findByText(/Link created successfully/i)).toBeTruthy();
    });

    it('blocks generate when shareReady is false', async () => {
        render(
            <ShareDialog
                file={baseFile}
                activeFolderId={7}
                shareReady={false}
                shareBlockedMessage="Bot not ready"
                onClose={vi.fn()}
            />,
        );

        const btn = screen.getByRole('button', { name: /Generate Shareable Link/i });
        expect(btn).toBeDisabled();
        fireEvent.click(btn);
        expect(invoke).not.toHaveBeenCalled();
    });
});
