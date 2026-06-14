import { motion } from 'framer-motion';
import { Download, Upload, Trash2, HardDrive, Globe, Key, Copy, Check, RefreshCw, FolderArchive, Activity, Sparkles } from 'lucide-react';
import { useSettings } from '../../../context/SettingsContext';
import type { GeneralTabProps } from './types';

export function GeneralTab(props: GeneralTabProps) {
    const {
        sessionOnline,
        transferBlockedMessage,
        apiSettings,
        apiHealth,
        apiHealthError,
        transportInfo,
        transportSwitching,
        apiPort,
        apiLoading,
        generatedKey,
        keyCopied,
        localPwdCopied,
        clearing,
        updateChecking,
        updateAvailable,
        updateVersion,
        updateDownloading,
        updateProgress,
        onApiToggle,
        onPortApply,
        onSetApiPort,
        onGenerateKey,
        onCopyKey,
        onCopyLocalPwd,
        onRegenerateLocalPwd,
        onClearCache,
        onCheckForUpdates,
        onInstallUpdate,
        onSwitchTransport,
    } = props;
    const { settings, updateSetting } = useSettings();

    return (
        <motion.div
            initial={{ opacity: 0, x: -20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: 20 }}
            transition={{ type: 'spring', damping: 25, stiffness: 220, opacity: { duration: 0.15 } }}
            className="space-y-6 w-full"
        >
            {/* Transfers Section */}
            <section className="space-y-3">
                <h3 className="text-xs font-semibold text-telegram-subtext uppercase tracking-wider flex items-center gap-2">
                    <Upload className="w-3.5 h-3.5" />
                    Transfers
                </h3>

                {/* Max Concurrent Uploads */}
                <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                    <div className="flex items-center gap-2">
                        <Upload className="w-4 h-4 text-telegram-subtext" />
                        <div>
                            <p className="text-sm text-telegram-text font-medium">Concurrent Uploads</p>
                            <p className="text-xs text-telegram-subtext">Max parallel uploads</p>
                        </div>
                    </div>
                    <div className="flex items-center gap-2">
                        <button
                            onClick={() => updateSetting('maxConcurrentUploads', Math.max(1, settings.maxConcurrentUploads - 1))}
                            className="w-7 h-7 flex items-center justify-center rounded-md bg-telegram-bg text-telegram-subtext hover:text-telegram-text hover:bg-telegram-border transition text-sm font-medium"
                        >
                            -
                        </button>
                        <span className="text-sm text-telegram-text font-medium w-5 text-center">
                            {settings.maxConcurrentUploads}
                        </span>
                        <button
                            onClick={() => updateSetting('maxConcurrentUploads', Math.min(10, settings.maxConcurrentUploads + 1))}
                            className="w-7 h-7 flex items-center justify-center rounded-md bg-telegram-bg text-telegram-subtext hover:text-telegram-text hover:bg-telegram-border transition text-sm font-medium"
                        >
                            +
                        </button>
                    </div>
                </div>

                {/* Max Concurrent Downloads */}
                <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                    <div className="flex items-center gap-2">
                        <Download className="w-4 h-4 text-telegram-subtext" />
                        <div>
                            <p className="text-sm text-telegram-text font-medium">Concurrent Downloads</p>
                            <p className="text-xs text-telegram-subtext">Max parallel downloads</p>
                        </div>
                    </div>
                    <div className="flex items-center gap-2">
                        <button
                            onClick={() => updateSetting('maxConcurrentDownloads', Math.max(1, settings.maxConcurrentDownloads - 1))}
                            className="w-7 h-7 flex items-center justify-center rounded-md bg-telegram-bg text-telegram-subtext hover:text-telegram-text hover:bg-telegram-border transition text-sm font-medium"
                        >
                            -
                        </button>
                        <span className="text-sm text-telegram-text font-medium w-5 text-center">
                            {settings.maxConcurrentDownloads}
                        </span>
                        <button
                            onClick={() => updateSetting('maxConcurrentDownloads', Math.min(10, settings.maxConcurrentDownloads + 1))}
                            className="w-7 h-7 flex items-center justify-center rounded-md bg-telegram-bg text-telegram-subtext hover:text-telegram-text hover:bg-telegram-border transition text-sm font-medium"
                        >
                            +
                        </button>
                    </div>
                </div>

                {/* Zip Folders */}
                <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                    <div className="flex items-center gap-2">
                        <FolderArchive className="w-4 h-4 text-telegram-subtext" />
                        <div>
                            <p className="text-sm text-telegram-text font-medium">Zip Folders Before Upload</p>
                            <p className="text-xs text-telegram-subtext">Compress folders into .zip before uploading</p>
                        </div>
                    </div>
                    <button
                        onClick={() => updateSetting('zipFolders', !settings.zipFolders)}
                        className={`relative w-11 h-6 rounded-full transition-colors duration-200 ${settings.zipFolders ? 'bg-telegram-primary' : 'bg-telegram-border'}`}
                    >
                        <span className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform duration-200 ${settings.zipFolders ? 'translate-x-5' : 'translate-x-0'}`} />
                    </button>
                </div>
            </section>

            {/* REST API Section */}
            <section className="space-y-3">
                <h3 className="text-xs font-semibold text-telegram-subtext uppercase tracking-wider flex items-center gap-2">
                    <Globe className="w-3.5 h-3.5" />
                    REST API
                </h3>

                {/* Enable Toggle */}
                <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                    <div className="flex items-center gap-2">
                        <div className={`w-2 h-2 rounded-full ${apiSettings.running ? 'bg-green-400 shadow-[0_0_6px_rgba(74,222,128,0.5)]' : 'bg-gray-500'}`} />
                        <div>
                            <p className="text-sm text-telegram-text font-medium">Enable API Server</p>
                            <p className="text-xs text-telegram-subtext">
                                {apiSettings.running ? `Running on port ${apiSettings.port}` : 'Localhost only (127.0.0.1)'}
                            </p>
                        </div>
                    </div>
                    <button
                        onClick={onApiToggle}
                        disabled={apiLoading}
                        className={`relative w-11 h-6 rounded-full transition-colors duration-200 ${apiSettings.enabled ? 'bg-telegram-primary' : 'bg-telegram-border'} disabled:opacity-50`}
                    >
                        <span className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform duration-200 ${apiSettings.enabled ? 'translate-x-5' : 'translate-x-0'}`} />
                    </button>
                </div>

                {apiSettings.running && (
                    <div className="p-3 rounded-lg bg-telegram-hover/50 space-y-1.5 text-xs text-telegram-subtext">
                        <div className="flex items-center gap-2 text-sm text-telegram-text font-medium">
                            <Activity className="w-3.5 h-3.5" />
                            API Health{apiHealth ? ` · v${apiHealth.version}` : ''}
                        </div>
                        <p>
                            App session:{' '}
                            <span className={sessionOnline ? 'text-green-400' : 'text-yellow-400'}>
                                {sessionOnline ? 'online (transfers allowed)' : (transferBlockedMessage || 'offline')}
                            </span>
                        </p>
                        {apiHealth ? (
                            <>
                                <p>
                                    Telegram:{' '}
                                    <span className={apiHealth.telegram_connected ? 'text-green-400' : 'text-yellow-400'}>
                                        {apiHealth.telegram_connected ? 'connected' : 'disconnected'}
                                    </span>
                                    {' · '}
                                    Ready: {apiHealth.ready ? 'yes' : 'no'}
                                    {' · '}
                                    Mode: {apiHealth.transport_mode || '—'}
                                </p>
                                <p>
                                    Upload queue — chunks free: {apiHealth.upload_queue?.chunk_slots_available ?? '?'}
                                    {' · '}
                                    files free: {apiHealth.upload_queue?.file_slots_available ?? '?'}
                                </p>
                                {transportInfo && transportInfo.available_modes.length > 0 && (
                                    <div className="flex flex-wrap gap-2 pt-1">
                                        {transportInfo.available_modes.map((mode) => (
                                            <button
                                                key={mode}
                                                type="button"
                                                disabled={
                                                    transportSwitching ||
                                                    transportInfo.active_mode === mode
                                                }
                                                onClick={() => onSwitchTransport(mode as 'bot' | 'user')}
                                                className="px-2 py-1 rounded border border-telegram-border text-telegram-text hover:bg-telegram-hover disabled:opacity-50 disabled:cursor-not-allowed"
                                            >
                                                {mode}
                                                {transportInfo.active_mode === mode ? ' (active)' : ''}
                                            </button>
                                        ))}
                                    </div>
                                )}
                            </>
                        ) : apiHealthError ? (
                            <p className="text-yellow-400">REST health unavailable: {apiHealthError}</p>
                        ) : (
                            <p>Fetching REST health…</p>
                        )}
                    </div>
                )}

                {/* Port */}
                <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                    <div>
                        <p className="text-sm text-telegram-text font-medium">Port</p>
                        <p className="text-xs text-telegram-subtext">1024 - 65535</p>
                    </div>
                    <div className="flex items-center gap-2">
                        <input
                            type="number"
                            min="1024"
                            max="65535"
                            value={apiPort}
                            onChange={e => onSetApiPort(e.target.value)}
                            onBlur={onPortApply}
                            onKeyDown={e => { if (e.key === 'Enter') onPortApply(); }}
                            className="w-20 bg-telegram-bg border border-telegram-border rounded-md px-2 py-1 text-sm text-telegram-text text-center focus:outline-none focus:border-telegram-primary/50 transition"
                        />
                    </div>
                </div>

                {apiSettings.enabled && apiSettings.local_access_pwd && (
                    <div className="p-3 rounded-lg bg-telegram-hover/50 space-y-2">
                        <div className="flex items-center justify-between">
                            <div>
                                <p className="text-sm text-telegram-text font-medium">Local Access Password</p>
                                <p className="text-xs text-telegram-subtext">Header <code className="text-telegram-primary">X-Access-Pwd</code> for curl (127.0.0.1)</p>
                            </div>
                            <button
                                type="button"
                                onClick={onRegenerateLocalPwd}
                                className="text-xs text-telegram-subtext hover:text-telegram-text"
                            >
                                Regenerate
                            </button>
                        </div>
                        <div className="flex items-center gap-2">
                            <code className="flex-1 text-xs bg-telegram-bg border border-telegram-border rounded px-2 py-1.5 font-mono truncate">
                                {apiSettings.local_access_pwd}
                            </code>
                            <button
                                type="button"
                                onClick={onCopyLocalPwd}
                                className="p-2 rounded-md bg-telegram-bg border border-telegram-border hover:bg-telegram-hover transition"
                                title="Copy password"
                            >
                                {localPwdCopied ? <Check className="w-4 h-4 text-green-400" /> : <Copy className="w-4 h-4" />}
                            </button>
                        </div>
                    </div>
                )}

                {/* API Key */}
                <div className="p-3 rounded-lg bg-telegram-hover/50 space-y-2.5">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                            <Key className="w-4 h-4 text-telegram-subtext" />
                            <div>
                                <p className="text-sm text-telegram-text font-medium">API Key</p>
                                <p className="text-xs text-telegram-subtext">
                                    {apiSettings.key_set ? 'Key configured' : 'No key set'}
                                </p>
                            </div>
                        </div>
                        <button
                            onClick={onGenerateKey}
                            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-telegram-primary/10 text-telegram-primary hover:bg-telegram-primary/20 transition"
                        >
                            <RefreshCw className="w-3 h-3" />
                            {apiSettings.key_set ? 'Regenerate' : 'Generate'}
                        </button>
                    </div>

                    {/* One-time key reveal */}
                    {generatedKey && (
                        <div className="mt-2 p-2.5 bg-telegram-bg rounded-lg border border-yellow-500/20 space-y-2">
                            <p className="text-[10px] text-yellow-400/80 uppercase tracking-wider font-semibold mb-1.5">
                                Copy now — this key will not be shown again
                            </p>
                            <div className="flex items-center gap-2">
                                <code className="flex-1 text-xs text-telegram-text font-mono bg-telegram-hover rounded px-2 py-1.5 overflow-x-auto select-all">
                                    {generatedKey}
                                </code>
                                <button
                                    onClick={onCopyKey}
                                    className="p-1.5 rounded-md hover:bg-telegram-hover text-telegram-subtext hover:text-telegram-text transition flex-shrink-0"
                                    title="Copy to clipboard"
                                >
                                    {keyCopied ? <Check className="w-4 h-4 text-green-400" /> : <Copy className="w-4 h-4" />}
                                </button>
                            </div>
                            <div className="pt-1">
                                <p className="text-[10px] text-telegram-subtext mb-1">Usage example:</p>
                                <code className="block text-[10px] text-telegram-text/70 font-mono bg-telegram-hover rounded px-2 py-1 select-all break-all">
                                    curl -H "X-API-Key: {generatedKey.substring(0, 16)}..." http://127.0.0.1:{apiSettings.port}/api/v1/health
                                </code>
                            </div>
                        </div>
                    )}
                </div>
            </section>

            {/* Storage Section */}
            <section className="space-y-3">
                <h3 className="text-xs font-semibold text-telegram-subtext uppercase tracking-wider flex items-center gap-2">
                    <HardDrive className="w-3.5 h-3.5" />
                    Storage
                </h3>

                <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                    <div className="flex items-center gap-2">
                        <Trash2 className="w-4 h-4 text-telegram-subtext" />
                        <div>
                            <p className="text-sm text-telegram-text font-medium">Clear Local Cache</p>
                            <p className="text-xs text-telegram-subtext">Remove cached previews and temp files</p>
                        </div>
                    </div>
                    <button
                        disabled={clearing}
                        onClick={onClearCache}
                        className="px-3 py-1.5 rounded-lg text-xs font-medium bg-red-500/10 text-red-400 hover:bg-red-500/20 transition disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                        {clearing ? 'Clearing...' : 'Clear'}
                    </button>
                </div>
            </section>

            {/* Updates Section */}
            <section className="space-y-3">
                <h3 className="text-xs font-semibold text-telegram-subtext uppercase tracking-wider flex items-center gap-2">
                    <Sparkles className="w-3.5 h-3.5" />
                    Updates
                </h3>

                <div className="p-3 rounded-lg bg-telegram-hover/50 space-y-3">
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                            <Sparkles className="w-4 h-4 text-telegram-subtext" />
                            <div>
                                <p className="text-sm text-telegram-text font-medium">Automatic Updates</p>
                                <p className="text-xs text-telegram-subtext">Check on startup when enabled</p>
                            </div>
                        </div>
                        <button
                            onClick={() => updateSetting('autoUpdate', !settings.autoUpdate)}
                            className={`relative w-11 h-6 rounded-full transition-colors duration-200 ${settings.autoUpdate ? 'bg-telegram-primary' : 'bg-telegram-border'}`}
                        >
                            <span className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform duration-200 ${settings.autoUpdate ? 'translate-x-5' : 'translate-x-0'}`} />
                        </button>
                    </div>
                    <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                            <Download className="w-4 h-4 text-telegram-subtext" />
                            <div>
                                <p className="text-sm text-telegram-text font-medium">Check for Updates</p>
                                <p className="text-xs text-telegram-subtext">
                                    {updateVersion ? `v${updateVersion} available` : 'Check if a newer version exists'}
                                </p>
                            </div>
                        </div>
                        {updateAvailable && !updateDownloading ? (
                            <button
                                onClick={onInstallUpdate}
                                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-telegram-primary text-white hover:bg-telegram-primary/90 transition"
                            >
                                <Download className="w-3 h-3" />
                                Update & Restart
                            </button>
                        ) : updateDownloading ? (
                            <div className="flex items-center gap-2">
                                <RefreshCw className="w-3.5 h-3.5 text-telegram-primary animate-spin" />
                                <span className="text-xs text-telegram-primary font-mono">{updateProgress}%</span>
                            </div>
                        ) : (
                            <button
                                onClick={onCheckForUpdates}
                                disabled={updateChecking}
                                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-telegram-primary/10 text-telegram-primary hover:bg-telegram-primary/20 transition disabled:opacity-50"
                            >
                                <RefreshCw className={`w-3 h-3 ${updateChecking ? 'animate-spin' : ''}`} />
                                {updateChecking ? 'Checking...' : 'Check Now'}
                            </button>
                        )}
                    </div>
                    {updateDownloading && (
                        <div className="w-full h-1.5 bg-telegram-border rounded-full overflow-hidden">
                            <div
                                className="h-full bg-telegram-primary rounded-full transition-all duration-300"
                                style={{ width: `${updateProgress}%` }}
                            />
                        </div>
                    )}
                </div>
            </section>
        </motion.div>
    );
}
