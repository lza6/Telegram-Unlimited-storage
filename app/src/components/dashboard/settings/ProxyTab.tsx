import { motion } from 'framer-motion';
import { Shield } from 'lucide-react';
import { useSettings } from '../../../context/SettingsContext';

export function ProxyTab() {
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
                <Shield className="w-3.5 h-3.5" />
                Proxy Configuration
            </h3>

            {/* Enable Proxy */}
            <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                <div className="flex items-center gap-2">
                    <div className={`w-2 h-2 rounded-full ${settings.proxyEnabled ? 'bg-green-400 shadow-[0_0_6px_rgba(74,222,128,0.5)]' : 'bg-gray-500'}`} />
                    <div>
                        <p className="text-sm text-telegram-text font-medium">Enable Proxy</p>
                        <p className="text-xs text-telegram-subtext">Route traffic through a proxy server</p>
                    </div>
                </div>
                <button
                    onClick={() => updateSetting('proxyEnabled', !settings.proxyEnabled)}
                    className={`relative w-11 h-6 rounded-full transition-colors duration-200 ${settings.proxyEnabled ? 'bg-telegram-primary' : 'bg-telegram-border'}`}
                >
                    <span className={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white shadow transition-transform duration-200 ${settings.proxyEnabled ? 'translate-x-5' : 'translate-x-0'}`} />
                </button>
            </div>

            {/* SOCKS5 only notice */}
            <div className="p-3 rounded-lg bg-yellow-500/5 border border-yellow-500/10">
                <p className="text-[11px] text-yellow-400/70 leading-relaxed">
                    SOCKS5 only. Changes apply immediately and trigger an automatic Telegram reconnect when logged in.
                </p>
            </div>

            {/* Host */}
            <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                <div>
                    <p className="text-sm text-telegram-text font-medium">Host</p>
                    <p className="text-xs text-telegram-subtext">Proxy server address</p>
                </div>
                <input
                    type="text"
                    placeholder="e.g. 127.0.0.1"
                    value={settings.proxyHost}
                    onChange={e => updateSetting('proxyHost', e.target.value)}
                    className="w-40 bg-telegram-bg border border-telegram-border rounded-md px-2 py-1 text-sm text-telegram-text text-right focus:outline-none focus:border-telegram-primary/50 transition placeholder:text-telegram-subtext/40"
                />
            </div>

            {/* Port */}
            <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                <div>
                    <p className="text-sm text-telegram-text font-medium">Port</p>
                    <p className="text-xs text-telegram-subtext">1–65535</p>
                </div>
                <input
                    type="number"
                    min="1"
                    max="65535"
                    value={settings.proxyPort}
                    onChange={e => updateSetting('proxyPort', Math.max(1, Math.min(65535, parseInt(e.target.value) || 1080)))}
                    className="w-20 bg-telegram-bg border border-telegram-border rounded-md px-2 py-1 text-sm text-telegram-text text-center focus:outline-none focus:border-telegram-primary/50 transition"
                />
            </div>

            {/* Username */}
            <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                <div>
                    <p className="text-sm text-telegram-text font-medium">Username</p>
                    <p className="text-xs text-telegram-subtext">Optional</p>
                </div>
                <input
                    type="text"
                    placeholder="Optional"
                    value={settings.proxyUsername}
                    onChange={e => updateSetting('proxyUsername', e.target.value)}
                    className="w-40 bg-telegram-bg border border-telegram-border rounded-md px-2 py-1 text-sm text-telegram-text text-right focus:outline-none focus:border-telegram-primary/50 transition placeholder:text-telegram-subtext/40"
                />
            </div>

            {/* Password */}
            <div className="flex items-center justify-between p-3 rounded-lg bg-telegram-hover/50">
                <div>
                    <p className="text-sm text-telegram-text font-medium">Password</p>
                    <p className="text-xs text-telegram-subtext">Optional</p>
                </div>
                <input
                    type="password"
                    placeholder="Optional"
                    value={settings.proxyPassword}
                    onChange={e => updateSetting('proxyPassword', e.target.value)}
                    className="w-40 bg-telegram-bg border border-telegram-border rounded-md px-2 py-1 text-sm text-telegram-text text-right focus:outline-none focus:border-telegram-primary/50 transition placeholder:text-telegram-subtext/40"
                />
            </div>
        </motion.section>
    );
}
