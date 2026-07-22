import { motion } from 'framer-motion';
import { Zap, Activity, Gauge, Wifi, ChevronDown } from 'lucide-react';
import { useSettings } from '../../../context/SettingsContext';
import { invoke } from '@tauri-apps/api/core';
import type { VpnTabProps } from './types';

export function VpnTab({ latencyMs, vpnDetected }: VpnTabProps) {
    const { settings, updateSetting } = useSettings();

    return (
        <motion.section
            initial={{ opacity: 0, x: -20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: 20 }}
            transition={{ type: 'spring', damping: 25, stiffness: 220, opacity: { duration: 0.15 } }}
            className="space-y-3 w-full"
        >
            <h3 className="text-xs font-semibold text-telegram-subtext uppercase tracking-wider flex items-center gap-2">
                <Zap className="w-3.5 h-3.5" />
                VPN Optimizer
                {latencyMs !== null && (
                    <span className={`ml-auto text-[10px] font-mono px-1.5 py-0.5 rounded-full ${
                        latencyMs < 0 ? 'bg-red-500/10 text-red-400' :
                        latencyMs < 100 ? 'bg-green-500/10 text-green-400' :
                        latencyMs < 300 ? 'bg-yellow-500/10 text-yellow-400' :
                        'bg-red-500/10 text-red-400'
                    }`}>
                        <Activity className="w-3 h-3 inline mr-0.5" />
                        {latencyMs < 0 ? 'Offline' : `${latencyMs}ms`}
                    </span>
                )}
            </h3>

            {/* Master Toggle */}
            <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                <div className="flex items-center gap-2">
                    <div className={`w-2 h-2 rounded-full ${settings.vpnMode ? 'bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.5)]' : 'bg-gray-500'}`} />
                    <div>
                        <p className="text-sm text-telegram-text font-medium">VPN Mode</p>
                        <p className="text-xs text-telegram-subtext">Optimize for high-latency / VPN connections</p>
                    </div>
                </div>
                <button
                    onClick={() => updateSetting('vpnMode', !settings.vpnMode)}
                    className={`relative w-11 h-6 rounded-full transition-colors duration-200 ${settings.vpnMode ? 'bg-emerald-500' : 'bg-telegram-border'}`}
                >
                    <span className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform duration-200 ${settings.vpnMode ? 'translate-x-5' : 'translate-x-0'}`} />
                </button>
            </div>

            {settings.vpnMode && (<>
                {/* Timeout Multiplier */}
                <div className="p-3 rounded-lg bg-telegram-hover/50 space-y-2">
                    <div className="flex items-center justify-between">
                        <div>
                            <p className="text-sm text-telegram-text font-medium">Timeout Multiplier</p>
                            <p className="text-xs text-telegram-subtext">Increase connection timeouts</p>
                        </div>
                        <span className="text-sm text-telegram-primary font-mono font-medium">{settings.timeoutMultiplier}×</span>
                    </div>
                    <input type="range" min="1" max="5" step="1" value={settings.timeoutMultiplier}
                        onChange={e => updateSetting('timeoutMultiplier', parseInt(e.target.value))}
                        className="w-full h-1.5 rounded-full appearance-none bg-telegram-border accent-telegram-primary cursor-pointer" />
                </div>

                {/* Retry Attempts */}
                <div className="p-3 rounded-lg bg-telegram-hover/50 space-y-2">
                    <div className="flex items-center justify-between">
                        <div>
                            <p className="text-sm text-telegram-text font-medium">Retry Attempts</p>
                            <p className="text-xs text-telegram-subtext">Retries on failed API calls</p>
                        </div>
                        <span className="text-sm text-telegram-primary font-mono font-medium">{settings.retryAttempts}</span>
                    </div>
                    <input type="range" min="0" max="5" step="1" value={settings.retryAttempts}
                        onChange={e => updateSetting('retryAttempts', parseInt(e.target.value))}
                        className="w-full h-1.5 rounded-full appearance-none bg-telegram-border accent-telegram-primary cursor-pointer" />
                </div>

                {/* Backoff Settings */}
                <div className="p-3 rounded-lg bg-telegram-hover/50 space-y-2">
                    <p className="text-sm text-telegram-text font-medium">Retry Backoff</p>
                    <div className="flex items-center justify-between">
                        <p className="text-xs text-telegram-subtext">Base delay</p>
                        <span className="text-xs text-telegram-primary font-mono">{settings.retryBaseBackoffSec}s</span>
                    </div>
                    <input type="range" min="0.5" max="5" step="0.5" value={settings.retryBaseBackoffSec}
                        onChange={e => updateSetting('retryBaseBackoffSec', parseFloat(e.target.value))}
                        className="w-full h-1.5 rounded-full appearance-none bg-telegram-border accent-telegram-primary cursor-pointer" />
                    <div className="flex items-center justify-between">
                        <p className="text-xs text-telegram-subtext">Max delay</p>
                        <span className="text-xs text-telegram-primary font-mono">{settings.retryMaxBackoffSec}s</span>
                    </div>
                    <input type="range" min="8" max="60" step="2" value={settings.retryMaxBackoffSec}
                        onChange={e => updateSetting('retryMaxBackoffSec', parseInt(e.target.value))}
                        className="w-full h-1.5 rounded-full appearance-none bg-telegram-border accent-telegram-primary cursor-pointer" />
                </div>

                {/* Adaptive Polling */}
                <div className="p-3 rounded-lg bg-telegram-hover/50 space-y-2">
                    <div className="flex items-center justify-between">
                        <div>
                            <p className="text-sm text-telegram-text font-medium">Adaptive Polling</p>
                            <p className="text-xs text-telegram-subtext">Auto-adjust update check interval</p>
                        </div>
                        <button
                            onClick={() => updateSetting('adaptivePolling', !settings.adaptivePolling)}
                            className={`relative w-11 h-6 rounded-full transition-colors duration-200 ${settings.adaptivePolling ? 'bg-telegram-primary' : 'bg-telegram-border'}`}
                        >
                            <span className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform duration-200 ${settings.adaptivePolling ? 'translate-x-5' : 'translate-x-0'}`} />
                        </button>
                    </div>
                    {settings.adaptivePolling && (<>
                        <div className="flex items-center justify-between">
                            <p className="text-xs text-telegram-subtext">Min interval</p>
                            <span className="text-xs text-telegram-primary font-mono">{settings.pollingMinSec}s</span>
                        </div>
                        <input type="range" min="10" max="30" step="5" value={settings.pollingMinSec}
                            onChange={e => updateSetting('pollingMinSec', parseInt(e.target.value))}
                            className="w-full h-1.5 rounded-full appearance-none bg-telegram-border accent-telegram-primary cursor-pointer" />
                        <div className="flex items-center justify-between">
                            <p className="text-xs text-telegram-subtext">Max interval</p>
                            <span className="text-xs text-telegram-primary font-mono">{settings.pollingMaxSec}s</span>
                        </div>
                        <input type="range" min="45" max="120" step="15" value={settings.pollingMaxSec}
                            onChange={e => updateSetting('pollingMaxSec', parseInt(e.target.value))}
                            className="w-full h-1.5 rounded-full appearance-none bg-telegram-border accent-telegram-primary cursor-pointer" />
                    </>)}
                </div>

                {/* Preferred DC */}
                <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                    <div>
                        <p className="text-sm text-telegram-text font-medium">Preferred Data Centre</p>
                        <p className="text-xs text-telegram-subtext">Start connections from this DC</p>
                    </div>
                    <div className="relative">
                        <select
                            value={settings.preferredDC}
                            onChange={e => updateSetting('preferredDC', e.target.value as typeof settings.preferredDC)}
                            className="appearance-none bg-telegram-bg border border-telegram-border rounded-md pl-3 pr-8 py-1.5 text-sm text-telegram-text focus:outline-none focus:border-telegram-primary/50 transition cursor-pointer"
                        >
                            <option value="auto">Auto</option>
                            <option value="dc1">DC 1</option>
                            <option value="dc2">DC 2</option>
                            <option value="dc3">DC 3</option>
                            <option value="dc4">DC 4</option>
                            <option value="dc5">DC 5</option>
                        </select>
                        <ChevronDown className="w-4 h-4 text-telegram-subtext absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none" />
                    </div>
                </div>

                {/* DC Fallback Attempts */}
                <div className="p-3 rounded-lg bg-telegram-hover/50 space-y-2">
                    <div className="flex items-center justify-between">
                        <div>
                            <p className="text-sm text-telegram-text font-medium">DC Fallback Attempts</p>
                            <p className="text-xs text-telegram-subtext">DCs to try on connection failure</p>
                        </div>
                        <span className="text-sm text-telegram-primary font-mono font-medium">{settings.dcFallbackAttempts}</span>
                    </div>
                    <input type="range" min="1" max="4" step="1" value={settings.dcFallbackAttempts}
                        onChange={e => updateSetting('dcFallbackAttempts', parseInt(e.target.value))}
                        className="w-full h-1.5 rounded-full appearance-none bg-telegram-border accent-telegram-primary cursor-pointer" />
                </div>

                {/* Flood Wait */}
                <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                    <div>
                        <p className="text-sm text-telegram-text font-medium">Respect Flood Wait</p>
                        <p className="text-xs text-telegram-subtext">Auto-sleep on FLOOD_WAIT errors</p>
                    </div>
                    <button
                        onClick={() => updateSetting('floodWaitRespect', !settings.floodWaitRespect)}
                        className={`relative w-11 h-6 rounded-full transition-colors duration-200 ${settings.floodWaitRespect ? 'bg-telegram-primary' : 'bg-telegram-border'}`}
                    >
                        <span className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform duration-200 ${settings.floodWaitRespect ? 'translate-x-5' : 'translate-x-0'}`} />
                    </button>
                </div>

                {/* Peer Cache Size */}
                <div className="p-3 rounded-lg bg-telegram-hover/50 space-y-2">
                    <div className="flex items-center justify-between">
                        <div>
                            <p className="text-sm text-telegram-text font-medium">Peer Cache Size</p>
                            <p className="text-xs text-telegram-subtext">Cached peer resolutions</p>
                        </div>
                        <span className="text-sm text-telegram-primary font-mono font-medium">{settings.peerCacheSize}</span>
                    </div>
                    <input type="range" min="100" max="2000" step="100" value={settings.peerCacheSize}
                        onChange={e => updateSetting('peerCacheSize', parseInt(e.target.value))}
                        className="w-full h-1.5 rounded-full appearance-none bg-telegram-border accent-telegram-primary cursor-pointer" />
                </div>

                {/* Bandwidth Throttle */}
                <div className="p-3 rounded-lg bg-telegram-hover/50 space-y-2">
                    <p className="text-sm text-telegram-text font-medium flex items-center gap-1.5">
                        <Gauge className="w-3.5 h-3.5 text-telegram-subtext" />
                        Bandwidth Throttle
                    </p>
                    <div className="flex items-center justify-between">
                        <p className="text-xs text-telegram-subtext">Upload limit</p>
                        <span className="text-xs text-telegram-primary font-mono">
                            {settings.bandwidthLimitUpKBs === 0 ? 'Unlimited' : `${settings.bandwidthLimitUpKBs} KB/s`}
                        </span>
                    </div>
                    <input type="range" min="0" max="5120" step="128" value={settings.bandwidthLimitUpKBs}
                        onChange={e => updateSetting('bandwidthLimitUpKBs', parseInt(e.target.value))}
                        className="w-full h-1.5 rounded-full appearance-none bg-telegram-border accent-telegram-primary cursor-pointer" />
                    <div className="flex items-center justify-between">
                        <p className="text-xs text-telegram-subtext">Download limit</p>
                        <span className="text-xs text-telegram-primary font-mono">
                            {settings.bandwidthLimitDownKBs === 0 ? 'Unlimited' : `${settings.bandwidthLimitDownKBs} KB/s`}
                        </span>
                    </div>
                    <input type="range" min="0" max="5120" step="128" value={settings.bandwidthLimitDownKBs}
                        onChange={e => updateSetting('bandwidthLimitDownKBs', parseInt(e.target.value))}
                        className="w-full h-1.5 rounded-full appearance-none bg-telegram-border accent-telegram-primary cursor-pointer" />
                </div>

                {/* Chunk Size */}
                <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                    <div>
                        <p className="text-sm text-telegram-text font-medium">Transfer Chunk Size</p>
                        <p className="text-xs text-telegram-subtext">Smaller = better for unstable connections</p>
                    </div>
                    <div className="relative">
                        <select
                            value={settings.chunkSizeKb}
                            onChange={e => updateSetting('chunkSizeKb', parseInt(e.target.value))}
                            className="appearance-none bg-telegram-bg border border-telegram-border rounded-md pl-3 pr-8 py-1.5 text-sm text-telegram-text focus:outline-none focus:border-telegram-primary/50 transition cursor-pointer"
                        >
                            <option value={128}>128 KB</option>
                            <option value={256}>256 KB</option>
                            <option value={512}>512 KB</option>
                        </select>
                        <ChevronDown className="w-4 h-4 text-telegram-subtext absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none" />
                    </div>
                </div>

                {/* Keep-Alive */}
                <div className="p-3 rounded-lg bg-telegram-hover/50 space-y-2">
                    <div className="flex items-center justify-between">
                        <div>
                            <p className="text-sm text-telegram-text font-medium">Keep-Alive Ping</p>
                            <p className="text-xs text-telegram-subtext">Prevent VPN idle disconnects</p>
                        </div>
                        <span className="text-sm text-telegram-primary font-mono font-medium">
                            {settings.keepAliveIntervalSec === 0 ? 'Off' : `${settings.keepAliveIntervalSec}s`}
                        </span>
                    </div>
                    <input type="range" min="0" max="120" step="15" value={settings.keepAliveIntervalSec}
                        onChange={e => updateSetting('keepAliveIntervalSec', parseInt(e.target.value))}
                        className="w-full h-1.5 rounded-full appearance-none bg-telegram-border accent-telegram-primary cursor-pointer" />
                </div>

                {/* Auto-Detect VPN */}
                <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                    <div className="flex items-center gap-2">
                        <Wifi className="w-4 h-4 text-telegram-subtext" />
                        <div>
                            <p className="text-sm text-telegram-text font-medium">Auto-Detect VPN</p>
                            <p className="text-xs text-telegram-subtext">
                                {vpnDetected === true ? 'VPN interface detected' : vpnDetected === false ? 'No VPN detected' : 'Checking...'}
                            </p>
                        </div>
                    </div>
                    <button
                        onClick={async () => {
                            const next = !settings.autoDetectVpn;
                            updateSetting('autoDetectVpn', next);
                            if (next) {
                                try {
                                    const found = await invoke<boolean>('cmd_detect_vpn');
                                    if (found && !settings.vpnMode) {
                                        updateSetting('vpnMode', true);
                                    }
                                } catch {
                                    // optional
                                }
                            }
                        }}
                        className={`relative w-11 h-6 rounded-full transition-colors duration-200 ${settings.autoDetectVpn ? 'bg-telegram-primary' : 'bg-telegram-border'}`}
                    >
                        <span className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform duration-200 ${settings.autoDetectVpn ? 'translate-x-5' : 'translate-x-0'}`} />
                    </button>
                </div>
            </>)}
        </motion.section>
    );
}
