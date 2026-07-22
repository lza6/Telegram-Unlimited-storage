import { createContext, useContext, useState, useEffect, ReactNode, useCallback } from 'react';
import { load } from '@tauri-apps/plugin-store';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';

export interface Settings {
    viewMode: 'grid' | 'list';
    autoUpdate: boolean;
    maxConcurrentUploads: number;
    maxConcurrentDownloads: number;
    zipFolders: boolean;
    /** Tailscale/LAN host override when copying share links */
    globalDomain: string;

    // ── Proxy ──────────────────────────────────────────────
    proxyEnabled: boolean;
    proxyType: 'socks5';
    proxyHost: string;
    proxyPort: number;
    proxyUsername: string;
    proxyPassword: string;   // SOCKS5
    proxySecret: string;     // MTProto

    // ── VPN Optimizer (master toggle) ─────────────────────
    vpnMode: boolean;

    // Individual controls (active only when vpnMode = true)
    timeoutMultiplier: number;       // 1–5
    retryAttempts: number;           // 0–5
    retryBaseBackoffSec: number;     // 0.5–5
    retryMaxBackoffSec: number;      // 8–60
    adaptivePolling: boolean;
    pollingMinSec: number;           // 10–30
    pollingMaxSec: number;           // 45–120
    preferredDC: 'auto' | 'dc1' | 'dc2' | 'dc3' | 'dc4' | 'dc5';
    dcFallbackAttempts: number;      // 1–4
    floodWaitRespect: boolean;
    peerCacheSize: number;           // 100–2000
    bandwidthLimitUpKBs: number;     // 0 = unlimited, KB/s
    bandwidthLimitDownKBs: number;   // 0 = unlimited, KB/s
    chunkSizeKb: number;             // 128, 256, 512
    keepAliveIntervalSec: number;    // 0 = disabled, 30–120
    autoDetectVpn: boolean;
}

const defaultSettings: Settings = {
    viewMode: 'grid',
    autoUpdate: true,
    maxConcurrentUploads: 6,
    maxConcurrentDownloads: 6,
    zipFolders: true,
    globalDomain: '',

    // Proxy — off by default
    proxyEnabled: false,
    proxyType: 'socks5',
    proxyHost: '',
    proxyPort: 1080,
    proxyUsername: '',
    proxyPassword: '',
    proxySecret: '',

    // VPN Optimizer — off by default (preserves existing behaviour)
    vpnMode: false,
    timeoutMultiplier: 3,
    retryAttempts: 3,
    retryBaseBackoffSec: 1,
    retryMaxBackoffSec: 30,
    adaptivePolling: true,
    pollingMinSec: 15,
    pollingMaxSec: 60,
    preferredDC: 'auto',
    dcFallbackAttempts: 2,
    floodWaitRespect: true,
    peerCacheSize: 500,
    bandwidthLimitUpKBs: 0,
    bandwidthLimitDownKBs: 0,
    chunkSizeKb: 512,
    keepAliveIntervalSec: 0,
    autoDetectVpn: false,
};

interface SettingsContextType {
    settings: Settings;
    updateSetting: <K extends keyof Settings>(key: K, value: Settings[K]) => void;
    resetSettings: () => void;
    isLoaded: boolean;
}

const SettingsContext = createContext<SettingsContextType | undefined>(undefined);

export function SettingsProvider({ children }: { children: ReactNode }) {
    const [settings, setSettings] = useState<Settings>(defaultSettings);
    const [isLoaded, setIsLoaded] = useState(false);

    // Load settings from Tauri store on mount
    useEffect(() => {
        const loadSettings = async () => {
            try {
                const store = await load('settings.json');
                const saved = await store.get<Settings>('settings');
                if (saved) {
                    // Merge with defaults so new keys are always present
                    setSettings({ ...defaultSettings, ...saved });
                }
                invoke<string>('cmd_get_ui_share_domain')
                    .then((domain) => {
                        if (domain?.trim()) {
                            setSettings((prev) => ({
                                ...prev,
                                globalDomain: domain.trim(),
                            }));
                        }
                    })
                    .catch(() => {});
            } catch {
                // Store not available or first run — use defaults
            } finally {
                setIsLoaded(true);
            }
        };
        loadSettings();
    }, []);

    // Merge persisted network_settings.json into UI settings on startup
    useEffect(() => {
        if (!isLoaded) return;
        invoke<{ proxy: { enabled: boolean; proxy_type: string; host: string; port: number; username: string; password: string; secret: string }; vpn: Record<string, unknown> }>('cmd_get_network_config')
            .then(async (snap) => {
                let vpnMode = Boolean(snap.vpn.enabled);
                const autoDetect = Boolean(snap.vpn.auto_detect_vpn);
                if (autoDetect && !vpnMode) {
                    try {
                        const found = await invoke<boolean>('cmd_detect_vpn');
                        if (found) {
                            vpnMode = true;
                            await invoke('cmd_apply_vpn_settings', {
                                enabled: true,
                                timeoutMultiplier: Number(snap.vpn.timeout_multiplier ?? 3),
                                retryAttempts: Number(snap.vpn.retry_attempts ?? 3),
                                retryBaseBackoffMs: Number(snap.vpn.retry_base_backoff_ms ?? 1000),
                                retryMaxBackoffMs: Number(snap.vpn.retry_max_backoff_ms ?? 30000),
                                adaptivePolling: snap.vpn.adaptive_polling !== false,
                                pollingMinSec: Number(snap.vpn.polling_min_sec ?? 15),
                                pollingMaxSec: Number(snap.vpn.polling_max_sec ?? 60),
                                preferredDc: String(snap.vpn.preferred_dc ?? 'auto'),
                                dcFallbackAttempts: Number(snap.vpn.dc_fallback_attempts ?? 2),
                                floodWaitRespect: snap.vpn.flood_wait_respect !== false,
                                peerCacheSize: Number(snap.vpn.peer_cache_size ?? 500),
                                bandwidthLimitUpKbs: Number(snap.vpn.bandwidth_limit_up_kbs ?? 0),
                                bandwidthLimitDownKbs: Number(snap.vpn.bandwidth_limit_down_kbs ?? 0),
                                chunkSizeKb: Number(snap.vpn.chunk_size_kb ?? 512),
                                keepAliveIntervalSec: Number(snap.vpn.keep_alive_interval_sec ?? 0),
                                autoDetectVpn: true,
                            });
                        }
                    } catch {
                        // optional auto-detect
                    }
                }
                setSettings((prev) => ({
                    ...prev,
                    proxyEnabled: snap.proxy.enabled,
                    proxyType: 'socks5',
                    proxyHost: snap.proxy.host,
                    proxyPort: snap.proxy.port,
                    proxyUsername: snap.proxy.username,
                    proxyPassword: snap.proxy.password,
                    vpnMode,
                    timeoutMultiplier: Number(snap.vpn.timeout_multiplier ?? prev.timeoutMultiplier),
                    retryAttempts: Number(snap.vpn.retry_attempts ?? prev.retryAttempts),
                    retryBaseBackoffSec: Number(snap.vpn.retry_base_backoff_ms ?? 1000) / 1000,
                    retryMaxBackoffSec: Number(snap.vpn.retry_max_backoff_ms ?? 30000) / 1000,
                    adaptivePolling: snap.vpn.adaptive_polling !== false,
                    pollingMinSec: Number(snap.vpn.polling_min_sec ?? prev.pollingMinSec),
                    pollingMaxSec: Number(snap.vpn.polling_max_sec ?? prev.pollingMaxSec),
                    preferredDC: (snap.vpn.preferred_dc as Settings['preferredDC']) || prev.preferredDC,
                    dcFallbackAttempts: Number(snap.vpn.dc_fallback_attempts ?? prev.dcFallbackAttempts),
                    floodWaitRespect: snap.vpn.flood_wait_respect !== false,
                    peerCacheSize: Number(snap.vpn.peer_cache_size ?? prev.peerCacheSize),
                    bandwidthLimitUpKBs: Number(snap.vpn.bandwidth_limit_up_kbs ?? 0),
                    bandwidthLimitDownKBs: Number(snap.vpn.bandwidth_limit_down_kbs ?? 0),
                    chunkSizeKb: Number(snap.vpn.chunk_size_kb ?? prev.chunkSizeKb),
                    keepAliveIntervalSec: Number(snap.vpn.keep_alive_interval_sec ?? 0),
                    autoDetectVpn: autoDetect,
                }));
            })
            .catch(() => {
                // network config optional on first run
            });
    }, [isLoaded]);

    const persistSettings = useCallback(async (next: Settings) => {
        try {
            const store = await load('settings.json');
            await store.set('settings', next);
            await store.save();
            invoke('cmd_set_ui_share_domain', { shareDomain: next.globalDomain }).catch((e) => {
                toast.error(`分享域名未能写入服务端配置: ${e}`);
            });
        } catch {
            toast.error('保存设置到磁盘失败');
        }
    }, []);

    const updateSetting = useCallback(<K extends keyof Settings>(key: K, value: Settings[K]) => {
        setSettings(prev => {
            const next = { ...prev, [key]: value };
            persistSettings(next);
            return next;
        });
    }, [persistSettings]);

    const resetSettings = useCallback(() => {
        setSettings(defaultSettings);
        persistSettings(defaultSettings);
        (async () => {
            try {
                await invoke('cmd_apply_proxy_settings', {
                    enabled: false,
                    proxyType: 'socks5',
                    host: '',
                    port: 1080,
                    username: '',
                    password: '',
                    secret: '',
                });
                await invoke('cmd_apply_vpn_settings', {
                    enabled: false,
                    timeoutMultiplier: defaultSettings.timeoutMultiplier,
                    retryAttempts: defaultSettings.retryAttempts,
                    retryBaseBackoffMs: Math.round(defaultSettings.retryBaseBackoffSec * 1000),
                    retryMaxBackoffMs: Math.round(defaultSettings.retryMaxBackoffSec * 1000),
                    adaptivePolling: defaultSettings.adaptivePolling,
                    pollingMinSec: defaultSettings.pollingMinSec,
                    pollingMaxSec: defaultSettings.pollingMaxSec,
                    preferredDc: defaultSettings.preferredDC,
                    dcFallbackAttempts: defaultSettings.dcFallbackAttempts,
                    floodWaitRespect: defaultSettings.floodWaitRespect,
                    peerCacheSize: defaultSettings.peerCacheSize,
                    bandwidthLimitUpKbs: defaultSettings.bandwidthLimitUpKBs,
                    bandwidthLimitDownKbs: defaultSettings.bandwidthLimitDownKBs,
                    chunkSizeKb: defaultSettings.chunkSizeKb,
                    keepAliveIntervalSec: defaultSettings.keepAliveIntervalSec,
                    autoDetectVpn: defaultSettings.autoDetectVpn,
                });
                await invoke('cmd_set_ui_share_domain', { shareDomain: '' });
            } catch {
                // best-effort network reset
            }
        })();
    }, [persistSettings]);

    return (
        <SettingsContext.Provider value={{ settings, updateSetting, resetSettings, isLoaded }}>
            {children}
        </SettingsContext.Provider>
    );
}

export const useSettings = () => {
    const context = useContext(SettingsContext);
    if (!context) throw new Error('useSettings must be used within a SettingsProvider');
    return context;
};
