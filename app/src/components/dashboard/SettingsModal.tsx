import { useState, useEffect, useCallback, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, RotateCcw, Globe, Shield, Zap, Link } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-shell';
import { toast } from 'sonner';
import { buildTelegramLoginCandidates } from '../../lib/filesPure';
import { check, Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { useSettings } from '../../context/SettingsContext';
import { useConfirm } from '../../context/ConfirmContext';
import { ShareInfo } from '../../types';
import { GeneralTab, ProxyTab, VpnTab, SharingTab } from './settings';
import type { ApiSettings, ApiHealthSnapshot, TransportInfoSnapshot } from './settings';

interface SettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
    sessionOnline?: boolean;
    shareReady?: boolean;
    transferBlockedMessage?: string;
    shareBlockedMessage?: string;
    onTransportSwitched?: () => void;
}

type SettingsTab = 'general' | 'proxy' | 'vpn' | 'sharing';

export function SettingsModal({
    isOpen,
    onClose,
    sessionOnline = false,
    shareReady = sessionOnline,
    transferBlockedMessage,
    shareBlockedMessage,
    onTransportSwitched,
}: SettingsModalProps) {
    const { settings, resetSettings } = useSettings();
    const { confirm } = useConfirm();
    const [clearing, setClearing] = useState(false);
    const [activeTab, setActiveTab] = useState<SettingsTab>('general');
    const [latencyMs, setLatencyMs] = useState<number | null>(null);
    const [vpnDetected, setVpnDetected] = useState<boolean | null>(null);

    // Update check state
    const [updateChecking, setUpdateChecking] = useState(false);
    const [updateAvailable, setUpdateAvailable] = useState<Update | null>(null);
    const [updateVersion, setUpdateVersion] = useState<string | null>(null);
    const [updateDownloading, setUpdateDownloading] = useState(false);
    const [updateProgress, setUpdateProgress] = useState(0);

    const handleCheckForUpdates = useCallback(async () => {
        setUpdateChecking(true);
        try {
            const updateInfo = await check();
            if (updateInfo) {
                setUpdateAvailable(updateInfo);
                setUpdateVersion(updateInfo.version);
                toast.success(`Update v${updateInfo.version} available!`);
            } else {
                setUpdateAvailable(null);
                setUpdateVersion(null);
                toast.success("You're on the latest version");
            }
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            if (msg.includes('dev') || msg.includes('no current version')) {
                toast.info('Update check is only available in production builds');
            } else {
                toast.error(`Update check failed: ${msg}`);
            }
        } finally {
            setUpdateChecking(false);
        }
    }, []);

    const handleInstallUpdate = useCallback(async () => {
        if (!updateAvailable) return;
        setUpdateDownloading(true);
        setUpdateProgress(0);
        let downloaded = 0;
        let contentLength = 0;
        try {
            await updateAvailable.downloadAndInstall((event) => {
                if (event.event === 'Started') {
                    const data = event.data as { contentLength?: number };
                    contentLength = data.contentLength || 0;
                } else if (event.event === 'Progress') {
                    const data = event.data as { chunkLength?: number };
                    downloaded += data.chunkLength || 0;
                    if (contentLength > 0) {
                        setUpdateProgress(Math.min(Math.round((downloaded / contentLength) * 100), 100));
                    }
                }
            });
            await relaunch();
        } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            toast.error(`Update failed: ${msg}`);
            setUpdateDownloading(false);
        }
    }, [updateAvailable]);

    // Sharing settings state
    const [shares, setShares] = useState<ShareInfo[]>([]);
    const [refreshing, setRefreshing] = useState(false);
    const [copiedId, setCopiedId] = useState<string | null>(null);
    const proxySyncedOnce = useRef(false);

    const fetchShares = useCallback(async () => {
        setRefreshing(true);
        try {
            const list = await invoke<ShareInfo[]>('cmd_list_shares');
            setShares(list);
        } catch (e) {
            toast.error(`Failed to load shares: ${e}`);
        } finally {
            setRefreshing(false);
        }
    }, []);

    useEffect(() => {
        if (isOpen && activeTab === 'sharing') {
            fetchShares();
        }
    }, [isOpen, activeTab, fetchShares]);

    useEffect(() => {
        if (!isOpen) return;
        const onInvalidate = () => {
            if (activeTab === 'sharing') fetchShares();
        };
        window.addEventListener('td-shares-invalidate', onInvalidate);
        return () => window.removeEventListener('td-shares-invalidate', onInvalidate);
    }, [isOpen, activeTab, fetchShares]);

    const handleRevokeShare = async (id: string) => {
        const ok = await confirm({
            title: 'Revoke Shareable Link',
            message: 'Are you sure you want to revoke this link? Anyone using it will no longer be able to download the file.',
            confirmText: 'Revoke',
            variant: 'danger',
        });
        if (!ok) return;

        try {
            await invoke('cmd_revoke_share', { id });
            toast.success('Shareable link revoked');
            fetchShares();
        } catch (e) {
            toast.error(`Failed to revoke link: ${e}`);
        }
    };

    const handleCopyShare = async (id: string) => {
        const share = shares.find(s => s.id === id);
        if (!share) return;

        let link = share.link;
        if (settings.globalDomain.trim()) {
            try {
                const url = new URL(link);
                link = `${url.protocol}//${settings.globalDomain.trim()}${url.pathname}`;
            } catch {
                link = `http://${settings.globalDomain.trim()}/d/${share.id}`;
            }
        }

        try {
            await navigator.clipboard.writeText(link);
            setCopiedId(share.id);
            setTimeout(() => setCopiedId(null), 2000);
        } catch {
            toast.error('复制失败，请手动选择链接');
        }
    };

    // API settings state
    const [apiSettings, setApiSettings] = useState<ApiSettings>({ enabled: false, port: 8550, key_set: false, running: false });
    const [apiHealth, setApiHealth] = useState<ApiHealthSnapshot | null>(null);
    const [apiHealthError, setApiHealthError] = useState<string | null>(null);
    const [transportInfo, setTransportInfo] = useState<TransportInfoSnapshot | null>(null);
    const [transportSwitching, setTransportSwitching] = useState(false);
    const [apiPort, setApiPort] = useState('8550');
    const [apiLoading, setApiLoading] = useState(false);
    const apiMutationInFlight = useRef(false);
    useEffect(() => {
        if (!apiLoading) apiMutationInFlight.current = false;
    }, [apiLoading]);
    const [generatedKey, setGeneratedKey] = useState<string | null>(null);
    const [keyCopied, setKeyCopied] = useState(false);
    const [localPwdCopied, setLocalPwdCopied] = useState(false);

    const fetchTransportInfo = useCallback(async () => {
        if (!apiSettings.running || !apiSettings.local_access_pwd) {
            setTransportInfo(null);
            return;
        }
        try {
            const res = await fetch(`http://127.0.0.1:${apiSettings.port}/api/v1/transport`, {
                headers: { 'X-Access-Pwd': apiSettings.local_access_pwd },
            });
            if (res.ok) {
                setTransportInfo(await res.json());
            } else {
                setTransportInfo(null);
            }
        } catch {
            setTransportInfo(null);
        }
    }, [apiSettings.running, apiSettings.port, apiSettings.local_access_pwd]);

    const fetchApiHealth = useCallback(async () => {
        try {
            const health = await invoke<ApiHealthSnapshot>('cmd_get_api_health');
            setApiHealth(health);
            setApiHealthError(null);
        } catch (e) {
            setApiHealth(null);
            setApiHealthError(String(e));
        }
    }, []);

    const fetchApiSettings = useCallback(async () => {
        try {
            const result = await invoke<ApiSettings>('cmd_get_api_settings');
            setApiSettings(result);
            setApiPort(result.port.toString());
            if (result.running) {
                await fetchApiHealth();
                await fetchTransportInfo();
            } else {
                setApiHealth(null);
                setApiHealthError(null);
                setTransportInfo(null);
            }
        } catch {
            // API settings not available (e.g. headless mode or dev)
            console.warn('API settings not available');
        }
    }, [fetchApiHealth, fetchTransportInfo]);

    const handleSwitchTransport = async (mode: 'bot' | 'user') => {
        const pwd = apiSettings.local_access_pwd;
        if (!pwd || !apiSettings.running) return;

        if (mode === 'user') {
            if (!await confirm({
                title: 'Switch to User mode',
                message:
                    'REST User 模式需在 Web 控制台完成 Telegram 登录（/telegram.html）。切换后请打开浏览器登录页绑定会话。确认切换？',
                confirmText: 'Switch',
            })) {
                return;
            }
        } else if (mode === 'bot') {
            if (!await confirm({
                title: 'Switch to Bot mode',
                message: '切换为 Bot 模式后无需 User 登录，但需配置 TG_BOT_TOKEN。确认切换？',
                confirmText: 'Switch',
            })) {
                return;
            }
        }

        setTransportSwitching(true);
        try {
            const res = await fetch(`http://127.0.0.1:${apiSettings.port}/api/v1/transport/mode`, {
                method: 'POST',
                headers: {
                    'X-Access-Pwd': pwd,
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({ mode }),
            });
            if (!res.ok) {
                const text = await res.text();
                throw new Error(text || res.statusText);
            }
            toast.success(`Transport mode switched to ${mode}`);
            try {
                await invoke<boolean>('cmd_invalidate_file_index');
            } catch {
                // REST handler already marks index incomplete; best-effort local sync
            }
            onTransportSwitched?.();
            toast.info('File index was reset — use Sync (desktop) or Refresh on Files page (Web) to rebuild.', { duration: 8000 });
            await fetchTransportInfo();
            await fetchApiHealth();
            if (mode === 'user') {
                const candidates = buildTelegramLoginCandidates(apiSettings.port, '/settings.html');
                let opened = false;
                for (const { surface, url } of candidates) {
                    const probeBase =
                        surface === 'desktop_api'
                            ? `http://127.0.0.1:${apiSettings.port}`
                            : 'http://127.0.0.1:1334';
                    let reachable = false;
                    try {
                        const probe = await fetch(`${probeBase}/telegram.html`, {
                            method: 'HEAD',
                            signal: AbortSignal.timeout(2500),
                        });
                        reachable = probe.ok;
                    } catch {
                        reachable = false;
                    }
                    if (!reachable) continue;
                    try {
                        await open(url);
                        opened = true;
                        const label =
                            surface === 'desktop_api'
                                ? `桌面 REST (:${apiSettings.port}/telegram.html)`
                                : 'Headless (:1334/telegram.html)';
                        toast.info(
                            `已在浏览器打开 ${label}。完成绑定后 REST User 模式即可就绪。`,
                            { duration: 8000 },
                        );
                        break;
                    } catch {
                        toast.info(`请手动打开 ${url} 完成 User 绑定。`, { duration: 8000 });
                        opened = true;
                        break;
                    }
                }
                if (!opened) {
                    toast.info(
                        '未检测到可用的 Web 登录页（8550 或 :1334）。请在本应用主界面完成 Telegram 登录，或启动 Headless Docker 后重试。',
                        { duration: 10000 },
                    );
                }
            }
        } catch (e) {
            toast.error(`Failed to switch transport: ${e}`);
        } finally {
            setTransportSwitching(false);
        }
    };

    // Load API settings when modal opens
    useEffect(() => {
        if (isOpen) {
            fetchApiSettings();
            setGeneratedKey(null);
            setKeyCopied(false);
        }
    }, [isOpen, fetchApiSettings]);

    // Poll API status while modal is open and API is enabled
    useEffect(() => {
        if (!isOpen || !apiSettings.enabled) return;
        const interval = setInterval(fetchApiSettings, 3000);
        return () => clearInterval(interval);
    }, [isOpen, apiSettings.enabled, fetchApiSettings]);

    // Sync proxy settings to backend when modal is open
    useEffect(() => {
        if (!isOpen) {
            proxySyncedOnce.current = false;
            return;
        }
        const applyProxy = async () => {
            try {
                await invoke('cmd_apply_proxy_settings', {
                    enabled: settings.proxyEnabled,
                    proxyType: 'socks5',
                    host: settings.proxyHost,
                    port: settings.proxyPort,
                    username: settings.proxyUsername,
                    password: settings.proxyPassword,
                    secret: '',
                });
                if (proxySyncedOnce.current) {
                    if (sessionOnline) {
                        try {
                            const ok = await invoke<boolean>('cmd_reconnect_telegram');
                            if (ok) toast.success('Proxy updated — Telegram reconnected');
                            else toast.info('Proxy saved (Telegram not logged in yet)');
                        } catch (e) {
                            toast.error(`Reconnect failed: ${e}`);
                        }
                    } else {
                        toast.info('Proxy saved — reconnect when Telegram session is active');
                    }
                } else {
                    proxySyncedOnce.current = true;
                }
            } catch (e) {
                toast.error(`Failed to sync proxy settings: ${e}`);
            }
        };
        applyProxy();
    }, [
        isOpen,
        sessionOnline,
        settings.proxyEnabled,
        settings.proxyHost,
        settings.proxyPort,
        settings.proxyUsername,
        settings.proxyPassword,
    ]);

    // Sync VPN optimizer settings when modal is open
    useEffect(() => {
        if (!isOpen) return;
        const applyVpn = async () => {
            try {
                await invoke('cmd_apply_vpn_settings', {
                    enabled: settings.vpnMode,
                    timeoutMultiplier: settings.timeoutMultiplier,
                    retryAttempts: settings.retryAttempts,
                    retryBaseBackoffMs: Math.round(settings.retryBaseBackoffSec * 1000),
                    retryMaxBackoffMs: Math.round(settings.retryMaxBackoffSec * 1000),
                    adaptivePolling: settings.adaptivePolling,
                    pollingMinSec: settings.pollingMinSec,
                    pollingMaxSec: settings.pollingMaxSec,
                    preferredDc: settings.preferredDC,
                    dcFallbackAttempts: settings.dcFallbackAttempts,
                    floodWaitRespect: settings.floodWaitRespect,
                    peerCacheSize: settings.peerCacheSize,
                    bandwidthLimitUpKbs: settings.bandwidthLimitUpKBs,
                    bandwidthLimitDownKbs: settings.bandwidthLimitDownKBs,
                    chunkSizeKb: settings.chunkSizeKb,
                    keepAliveIntervalSec: settings.keepAliveIntervalSec,
                    autoDetectVpn: settings.autoDetectVpn,
                });
            } catch (e) {
                toast.error(`Failed to sync VPN settings: ${e}`);
            }
        };
        applyVpn();
    }, [
        isOpen,
        settings.vpnMode,
        settings.timeoutMultiplier,
        settings.retryAttempts,
        settings.retryBaseBackoffSec,
        settings.retryMaxBackoffSec,
        settings.adaptivePolling,
        settings.pollingMinSec,
        settings.pollingMaxSec,
        settings.preferredDC,
        settings.dcFallbackAttempts,
        settings.floodWaitRespect,
        settings.peerCacheSize,
        settings.bandwidthLimitUpKBs,
        settings.bandwidthLimitDownKBs,
        settings.chunkSizeKb,
        settings.keepAliveIntervalSec,
        settings.autoDetectVpn,
    ]);

    // Poll latency when VPN tab is active
    useEffect(() => {
        if (!isOpen || activeTab !== 'vpn') return;
        const check = async () => {
            try {
                const ms = await invoke<number>('cmd_check_latency');
                setLatencyMs(ms);
            } catch { setLatencyMs(null); }
        };
        check();
        const interval = setInterval(check, 5000);
        return () => clearInterval(interval);
    }, [isOpen, activeTab]);

    // Detect VPN interfaces when VPN tab opens
    useEffect(() => {
        if (!isOpen || activeTab !== 'vpn') return;
        const detect = async () => {
            try {
                const found = await invoke<boolean>('cmd_detect_vpn');
                setVpnDetected(found);
            } catch { setVpnDetected(null); }
        };
        detect();
    }, [isOpen, activeTab]);

    const handleApiToggle = async () => {
        if (apiMutationInFlight.current) return;
        apiMutationInFlight.current = true;
        setApiLoading(true);
        try {
            const port = parseInt(apiPort, 10);
            if (isNaN(port) || port < 1024 || port > 65535) {
                toast.error('Port must be between 1024 and 65535');
                setApiLoading(false);
                return;
            }
            const result = await invoke<ApiSettings>('cmd_update_api_settings', {
                enabled: !apiSettings.enabled,
                port,
            });
            setApiSettings(result);
            toast.success(result.enabled ? 'API server started' : 'API server stopped');
            if (result.enabled) {
                setTimeout(() => fetchApiSettings(), 500);
            }
        } catch (e) {
            toast.error(`Failed to update API: ${e}`);
        } finally {
            setApiLoading(false);
        }
    };

    const handlePortApply = async () => {
        if (apiMutationInFlight.current) return;
        const port = parseInt(apiPort, 10);
        if (isNaN(port) || port < 1024 || port > 65535) {
            toast.error('Port must be between 1024 and 65535');
            return;
        }
        if (port === apiSettings.port) return;
        apiMutationInFlight.current = true;
        setApiLoading(true);
        try {
            const result = await invoke<ApiSettings>('cmd_update_api_settings', {
                enabled: apiSettings.enabled,
                port,
            });
            setApiSettings(result);
            toast.success(`API port updated to ${port}`);
        } catch (e) {
            toast.error(`Failed to update port: ${e}`);
        } finally {
            setApiLoading(false);
        }
    };

    const handleGenerateKey = async () => {
        if (apiMutationInFlight.current) return;
        apiMutationInFlight.current = true;
        setApiLoading(true);
        const ok = await confirm({
            title: 'Generate API Key',
            message: apiSettings.key_set
                ? 'This will revoke your current API key and generate a new one. Any existing integrations will stop working.'
                : 'Generate a new API key for authenticating REST API requests.',
            confirmText: apiSettings.key_set ? 'Regenerate' : 'Generate',
            variant: apiSettings.key_set ? 'danger' : 'info',
        });
        if (!ok) {
            setApiLoading(false);
            return;
        }
        try {
            const key = await invoke<string>('cmd_regenerate_api_key');
            setGeneratedKey(key);
            setKeyCopied(false);
            setApiSettings(prev => ({ ...prev, key_set: true }));
            toast.success('API key generated');
            setApiLoading(false);
        } catch (e) {
            toast.error(`Failed to generate key: ${e}`);
            setApiLoading(false);
        }
    };

    const handleCopyKey = async () => {
        if (!generatedKey) return;
        try {
            await navigator.clipboard.writeText(generatedKey);
            setKeyCopied(true);
            setTimeout(() => setKeyCopied(false), 2000);
        } catch {
            toast.error('Failed to copy to clipboard');
        }
    };

    const handleCopyLocalPwd = async () => {
        const pwd = apiSettings.local_access_pwd;
        if (!pwd) return;
        try {
            await navigator.clipboard.writeText(pwd);
            setLocalPwdCopied(true);
            setTimeout(() => setLocalPwdCopied(false), 2000);
        } catch {
            toast.error('Failed to copy to clipboard');
        }
    };

    const handleRegenerateLocalPwd = async () => {
        if (apiMutationInFlight.current) return;
        apiMutationInFlight.current = true;
        setApiLoading(true);
        const ok = await confirm({
            title: 'Regenerate Local Access Password',
            message: 'Scripts using the current X-Access-Pwd header will stop working until updated.',
            confirmText: 'Regenerate',
            variant: 'danger',
        });
        if (!ok) {
            setApiLoading(false);
            return;
        }
        try {
            const pwd = await invoke<string>('cmd_regenerate_local_access_pwd');
            setApiSettings(prev => ({ ...prev, local_access_pwd: pwd }));
            toast.success('Local access password regenerated');
            setApiLoading(false);
        } catch (e) {
            toast.error(`Failed: ${e}`);
            setApiLoading(false);
        }
    };

    const handleClearCache = async () => {
        const ok = await confirm({
            title: 'Clear Cache',
            message: 'This will remove all cached previews and temporary files. Your uploaded files on Telegram are not affected.',
            confirmText: 'Clear',
            variant: 'danger',
        });
        if (!ok) return;
        setClearing(true);
        try {
            await invoke('cmd_clean_cache');
            toast.success('Cache cleared successfully');
        } catch {
            toast.error('Failed to clear cache');
        } finally {
            setClearing(false);
        }
    };

    return (
        <AnimatePresence>
            {isOpen && (
                <motion.div
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    exit={{ opacity: 0 }}
                    className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 backdrop-blur-sm"
                    onClick={onClose}
                >
                    <motion.div
                        layout
                        initial={{ opacity: 0, scale: 0.95, y: 10 }}
                        animate={{ opacity: 1, scale: 1, y: 0 }}
                        exit={{ opacity: 0, scale: 0.95, y: 10 }}
                        transition={{ type: 'spring', damping: 25, stiffness: 220 }}
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="settings-title"
                        className="bg-telegram-surface border border-telegram-border rounded-xl w-full max-w-[440px] mx-4 shadow-2xl overflow-hidden flex flex-col"
                        onClick={e => e.stopPropagation()}
                    >
                        {/* Header */}
                        <div className="px-5 py-4 border-b border-telegram-border flex justify-between items-center">
                            <h2 id="settings-title" className="text-telegram-text font-semibold text-base">Settings</h2>
                            <button
                                onClick={onClose}
                                className="p-1.5 hover:bg-telegram-hover rounded-lg text-telegram-subtext hover:text-telegram-text transition"
                            >
                                <X className="w-4 h-4" />
                            </button>
                        </div>

                        {/* Tab Bar */}
                        <div className="px-5 pt-3 pb-0 flex gap-1 border-b border-telegram-border">
                            {([['general', 'General', Globe], ['proxy', 'Proxy', Shield], ['vpn', 'VPN', Zap], ['sharing', 'Sharing', Link]] as const).map(([key, label, Icon]) => (
                                <button
                                    key={key}
                                    onClick={() => setActiveTab(key as SettingsTab)}
                                    className={`flex items-center gap-1.5 px-3 py-2 text-xs font-medium rounded-t-lg transition-colors ${
                                        activeTab === key
                                            ? 'text-telegram-primary border-b-2 border-telegram-primary bg-telegram-primary/5'
                                            : 'text-telegram-subtext hover:text-telegram-text hover:bg-telegram-hover/50'
                                    }`}
                                >
                                    <Icon className="w-3.5 h-3.5" />
                                    {label}
                                </button>
                            ))}
                        </div>

                        {/* Body */}
                        <motion.div layout className="px-5 py-4 max-h-[70vh] overflow-y-auto overflow-x-hidden relative">
                            <AnimatePresence mode="popLayout" initial={false}>
                                {activeTab === 'general' && (
                                    <GeneralTab
                                        sessionOnline={sessionOnline}
                                        transferBlockedMessage={transferBlockedMessage}
                                        apiSettings={apiSettings}
                                        apiHealth={apiHealth}
                                        apiHealthError={apiHealthError}
                                        transportInfo={transportInfo}
                                        transportSwitching={transportSwitching}
                                        apiPort={apiPort}
                                        apiLoading={apiLoading}
                                        generatedKey={generatedKey}
                                        keyCopied={keyCopied}
                                        localPwdCopied={localPwdCopied}
                                        clearing={clearing}
                                        updateChecking={updateChecking}
                                        updateAvailable={!!updateAvailable}
                                        updateVersion={updateVersion}
                                        updateDownloading={updateDownloading}
                                        updateProgress={updateProgress}
                                        onApiToggle={handleApiToggle}
                                        onPortApply={handlePortApply}
                                        onSetApiPort={setApiPort}
                                        onGenerateKey={handleGenerateKey}
                                        onCopyKey={handleCopyKey}
                                        onCopyLocalPwd={handleCopyLocalPwd}
                                        onRegenerateLocalPwd={handleRegenerateLocalPwd}
                                        onClearCache={handleClearCache}
                                        onCheckForUpdates={handleCheckForUpdates}
                                        onInstallUpdate={handleInstallUpdate}
                                        onSwitchTransport={handleSwitchTransport}
                                    />
                                )}

                                {activeTab === 'proxy' && <ProxyTab />}

                                {activeTab === 'vpn' && (
                                    <VpnTab
                                        latencyMs={latencyMs}
                                        vpnDetected={vpnDetected}
                                    />
                                )}

                                {activeTab === 'sharing' && (
                                    <SharingTab
                                        shareReady={shareReady}
                                        shareBlockedMessage={shareBlockedMessage || transferBlockedMessage}
                                        shares={shares}
                                        refreshing={refreshing}
                                        copiedId={copiedId}
                                        onFetchShares={fetchShares}
                                        onRevokeShare={handleRevokeShare}
                                        onCopyShare={handleCopyShare}
                                    />
                                )}
                            </AnimatePresence>
                        </motion.div>

                        {/* Footer */}
                        <div className="px-5 py-3 border-t border-telegram-border flex items-center justify-between">
                            <button
                                onClick={resetSettings}
                                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs text-telegram-subtext hover:text-red-400 hover:bg-red-500/10 transition font-medium"
                            >
                                <RotateCcw className="w-3.5 h-3.5" />
                                Reset to Defaults
                            </button>
                            <button
                                onClick={onClose}
                                className="px-4 py-1.5 rounded-lg text-xs font-medium bg-telegram-primary text-white hover:bg-telegram-primary/90 transition"
                            >
                                Done
                            </button>
                        </div>
                    </motion.div>
                </motion.div>
            )}
        </AnimatePresence>
    );
}
