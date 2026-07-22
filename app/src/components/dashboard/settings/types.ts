import { ShareInfo } from '../../../types';

export interface ApiSettings {
    enabled: boolean;
    port: number;
    key_set: boolean;
    running: boolean;
    local_access_pwd?: string | null;
}

export interface ApiHealthSnapshot {
    status: string;
    version: string;
    telegram_connected: boolean;
    ready: boolean;
    transport_mode: string;
    upload_queue: {
        chunk_slots_available?: number;
        file_slots_available?: number;
    };
}

export interface TransportInfoSnapshot {
    active_mode: string;
    available_modes: string[];
    bot_configured?: boolean;
    user_configured?: boolean;
}

export interface GeneralTabProps {
    sessionOnline: boolean;
    transferBlockedMessage?: string;
    apiSettings: ApiSettings;
    apiHealth: ApiHealthSnapshot | null;
    apiHealthError: string | null;
    transportInfo: TransportInfoSnapshot | null;
    transportSwitching: boolean;
    apiPort: string;
    apiLoading: boolean;
    generatedKey: string | null;
    keyCopied: boolean;
    localPwdCopied: boolean;
    clearing: boolean;
    updateChecking: boolean;
    updateAvailable: boolean;
    updateVersion: string | null;
    updateDownloading: boolean;
    updateProgress: number;
    onApiToggle: () => void;
    onPortApply: () => void;
    onSetApiPort: (port: string) => void;
    onGenerateKey: () => void;
    onCopyKey: () => void;
    onCopyLocalPwd: () => void;
    onRegenerateLocalPwd: () => void;
    onClearCache: () => void;
    onCheckForUpdates: () => void;
    onInstallUpdate: () => void;
    onSwitchTransport: (mode: 'bot' | 'user') => void;
}

export interface ProxyTabProps {
    sessionOnline: boolean;
}

export interface VpnTabProps {
    latencyMs: number | null;
    vpnDetected: boolean | null;
}

export interface SharingTabProps {
    shareReady: boolean;
    shareBlockedMessage?: string;
    shares: ShareInfo[];
    refreshing: boolean;
    copiedId: string | null;
    onFetchShares: () => void;
    onRevokeShare: (id: string) => void;
    onCopyShare: (id: string) => void;
}
