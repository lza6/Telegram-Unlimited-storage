import type { ComponentProps } from 'react';
import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { GeneralTab } from './GeneralTab';

vi.mock('../../../context/SettingsContext', () => ({
    useSettings: () => ({
        settings: { maxConcurrentUploads: 2, maxConcurrentDownloads: 2, zipFolders: false, autoUpdate: false },
        updateSetting: vi.fn(),
    }),
}));

const apiSettings = { enabled: true, port: 8550, key_set: true, running: true, local_access_pwd: 'local-password' };

function props(overrides: Partial<ComponentProps<typeof GeneralTab>> = {}): ComponentProps<typeof GeneralTab> {
    return {
        sessionOnline: true, apiSettings, apiHealth: null, apiHealthError: null, transportInfo: null, transportSwitching: false,
        apiPort: '8550', apiLoading: true, generatedKey: null, keyCopied: false, localPwdCopied: false, clearing: false,
        updateChecking: false, updateAvailable: false, updateVersion: null, updateDownloading: false, updateProgress: 0,
        onApiToggle: vi.fn(), onPortApply: vi.fn(), onSetApiPort: vi.fn(), onGenerateKey: vi.fn(), onCopyKey: vi.fn(),
        onCopyLocalPwd: vi.fn(), onRegenerateLocalPwd: vi.fn(), onClearCache: vi.fn(), onCheckForUpdates: vi.fn(),
        onInstallUpdate: vi.fn(), onSwitchTransport: vi.fn(), ...overrides,
    };
}

describe('GeneralTab API mutation controls', () => {
    it('disables key/password mutation controls while an API action is in flight', () => {
        render(<GeneralTab {...props()} />);

        expect(screen.getByRole('button', { name: /Regenerate API key/i })).toBeDisabled();
        expect(screen.getByRole('button', { name: 'Regenerate local access password' })).toBeDisabled();
        expect(screen.getByRole('spinbutton', { name: 'API server port' })).toBeDisabled();
    });
});
