import { motion } from 'framer-motion';
import { Link, RefreshCw, Copy, Check, Trash2, Key } from 'lucide-react';
import { useSettings } from '../../../context/SettingsContext';
import type { SharingTabProps } from './types';

export function SharingTab({
    shareReady,
    shareBlockedMessage,
    shares,
    refreshing,
    copiedId,
    onFetchShares,
    onRevokeShare,
    onCopyShare,
}: SharingTabProps) {
    const { settings, updateSetting } = useSettings();

    return (
        <motion.section
            initial={{ opacity: 0, x: -20 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: 20 }}
            transition={{ type: 'spring', damping: 25, stiffness: 220, opacity: { duration: 0.15 } }}
            className="space-y-4 w-full"
        >
            <div className="flex items-center justify-between">
                <h3 className="text-xs font-semibold text-telegram-subtext uppercase tracking-wider flex items-center gap-2">
                    <Link className="w-3.5 h-3.5 text-telegram-primary" />
                    Shared Links ({shares.length})
                </h3>
                <button
                    onClick={onFetchShares}
                    className="text-telegram-subtext hover:text-telegram-text p-1 rounded hover:bg-telegram-hover transition"
                    title="Refresh links"
                >
                    <RefreshCw className={`w-3.5 h-3.5 ${refreshing ? 'animate-spin' : ''}`} />
                </button>
            </div>

            {!shareReady && (
                <div className="text-xs text-yellow-200/90 bg-yellow-500/10 border border-yellow-500/30 rounded-lg px-3 py-2">
                    {shareBlockedMessage || 'Share unavailable until service is ready'} — listed links can still be copied or revoked (local DB).
                </div>
            )}

            <div className="bg-telegram-hover/30 border border-telegram-border/50 rounded-lg p-3 space-y-2">
                <div className="text-[11px] font-semibold text-telegram-text flex items-center gap-1">🌐 Tailscale/LAN IP Override</div>
                <input
                    type="text"
                    placeholder="e.g. 100.115.22.45 or my-pc:14201"
                    value={settings.globalDomain}
                    onChange={(e) => updateSetting('globalDomain', e.target.value)}
                    className="w-full bg-telegram-surface border border-telegram-border rounded-md px-2.5 py-1.5 text-xs text-telegram-text focus:outline-none focus:border-telegram-primary/50 placeholder:text-telegram-subtext/40"
                />
                <p className="text-[10px] text-telegram-subtext">
                    Automatically replaces 'localhost:14201' with this IP/domain when copying.
                </p>
            </div>

            {shares.length === 0 ? (
                <div className="py-8 text-center space-y-2">
                    <Link className="w-8 h-8 text-telegram-subtext/40 mx-auto" />
                    <p className="text-sm font-medium text-telegram-text">No active share links</p>
                    <p className="text-xs text-telegram-subtext">Right-click any file and select "Share Link" to create one.</p>
                </div>
            ) : (
                <div className="space-y-2 max-h-[300px] overflow-y-auto pr-1 custom-scrollbar">
                    {shares.map((share) => {
                        const isExpired = share.expires_at ? (share.expires_at < Math.floor(Date.now() / 1000)) : false;
                        return (
                            <div key={share.id} className="p-3 rounded-lg bg-telegram-hover/40 border border-telegram-border/50 flex flex-col gap-2 relative">
                                <div className="flex justify-between items-start gap-4">
                                    <div className="min-w-0 flex-1">
                                        <div className="text-xs font-semibold text-telegram-text truncate" title={share.file_name}>
                                            {share.file_name}
                                        </div>
                                        <div className="flex gap-2 items-center mt-1 flex-wrap text-[10px]">
                                            <span className="text-telegram-subtext">
                                                {new Date(share.created_at * 1000).toLocaleDateString()}
                                            </span>
                                            <span className="w-1 h-1 rounded-full bg-telegram-border" />
                                            {share.has_password ? (
                                                <span className="text-emerald-400 bg-emerald-500/10 px-1.5 py-0.5 rounded flex items-center gap-0.5 font-medium">
                                                    <Key className="w-2.5 h-2.5" /> Protected
                                                </span>
                                            ) : (
                                                <span className="text-blue-400 bg-blue-500/10 px-1.5 py-0.5 rounded font-medium">Public</span>
                                            )}
                                            <span className="w-1 h-1 rounded-full bg-telegram-border" />
                                            {share.expires_at ? (
                                                isExpired ? (
                                                    <span className="text-red-400 bg-red-500/10 px-1.5 py-0.5 rounded font-medium">Expired</span>
                                                ) : (
                                                    <span className="text-amber-400 bg-amber-500/10 px-1.5 py-0.5 rounded font-medium">
                                                        Expires: {new Date(share.expires_at * 1000).toLocaleDateString()}
                                                    </span>
                                                )
                                            ) : (
                                                <span className="text-teal-400 bg-teal-500/10 px-1.5 py-0.5 rounded font-medium">Never Expires</span>
                                            )}
                                        </div>
                                    </div>

                                    <div className="flex gap-1">
                                        <button
                                            onClick={() => onCopyShare(share.id)}
                                            className={`p-1.5 rounded bg-telegram-surface border border-telegram-border text-telegram-text hover:bg-telegram-hover transition ${
                                                copiedId === share.id ? 'text-emerald-400 border-emerald-500/30 bg-emerald-500/5' : ''
                                            }`}
                                            title="Copy share link"
                                        >
                                            {copiedId === share.id ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
                                        </button>
                                        <button
                                            onClick={() => onRevokeShare(share.id)}
                                            className="p-1.5 rounded bg-telegram-surface border border-telegram-border text-red-400 hover:bg-red-500/10 hover:border-red-500/30 transition"
                                            title="Revoke link"
                                        >
                                            <Trash2 className="w-3.5 h-3.5" />
                                        </button>
                                    </div>
                                </div>
                            </div>
                        );
                    })}
                </div>
            )}
        </motion.section>
    );
}
