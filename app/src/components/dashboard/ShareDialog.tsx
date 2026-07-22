import { useState } from 'react';
import { Plus, Link, Copy, Check, Shield, Clock, AlertCircle } from 'lucide-react';
import { TelegramFile, ShareInfo } from '../../types';
import { invoke } from '@tauri-apps/api/core';
import { motion, AnimatePresence } from 'framer-motion';

interface ShareDialogProps {
    file: TelegramFile;
    onClose: () => void;
}

export function ShareDialog({ file, onClose }: ShareDialogProps) {
    const [password, setPassword] = useState('');
    const [requirePassword, setRequirePassword] = useState(false);
    const [expiryType, setExpiryType] = useState<'never' | '1h' | '1d' | '7d' | 'custom'>('1d');
    const [customHours, setCustomHours] = useState('24');
    
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [shareInfo, setShareInfo] = useState<ShareInfo | null>(null);
    const [copied, setCopied] = useState(false);
    const [customDomain, setCustomDomain] = useState('');

    const handleGenerate = async () => {
        setLoading(true);
        setError(null);
        try {
            let expiryHours: number | null = null;
            if (expiryType === '1h') expiryHours = 1;
            else if (expiryType === '1d') expiryHours = 24;
            else if (expiryType === '7d') expiryHours = 168;
            else if (expiryType === 'custom') {
                const parsed = parseInt(customHours, 10);
                if (isNaN(parsed) || parsed <= 0) {
                    throw new Error('Please enter a valid number of hours');
                }
                expiryHours = parsed;
            }

            const pwdParam = requirePassword && password.trim() ? password : null;

            const res = await invoke<ShareInfo>('cmd_create_share', {
                folderId: null, // Always file-level for now
                messageId: file.id, // In Telegram Drive, file.id is the message id
                fileName: file.name,
                fileSize: file.size,
                password: pwdParam,
                expiryHours,
            });

            setShareInfo(res);
        } catch (err: any) {
            setError(err.toString());
        } finally {
            setLoading(false);
        }
    };

    const getDisplayLink = () => {
        if (!shareInfo) return '';
        if (customDomain.trim()) {
            try {
                // Replace the host part (localhost:14201) with the custom domain
                const url = new URL(shareInfo.link);
                return `${url.protocol}//${customDomain.trim()}${url.pathname}`;
            } catch {
                return shareInfo.link;
            }
        }
        return shareInfo.link;
    };

    const handleCopy = () => {
        const link = getDisplayLink();
        if (link) {
            navigator.clipboard.writeText(link);
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        }
    };

    return (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
            <div className="bg-telegram-surface border border-telegram-border rounded-xl w-[420px] shadow-2xl overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-150" onClick={e => e.stopPropagation()}>
                <div className="p-4 border-b border-telegram-border flex justify-between items-center">
                    <h3 className="text-telegram-text font-medium flex items-center gap-2">
                        <Link className="w-5 h-5 text-telegram-primary" />
                        Share File
                    </h3>
                    <button onClick={onClose} className="text-telegram-subtext hover:text-telegram-text">
                        <Plus className="w-5 h-5 rotate-45" />
                    </button>
                </div>

                <div className="p-5 flex-1 overflow-y-auto space-y-4 max-h-[75vh]">
                    <div className="bg-telegram-hover/40 border border-telegram-border/50 rounded-lg p-3">
                        <div className="text-xs text-telegram-subtext uppercase font-semibold tracking-wider mb-1">Sharing File</div>
                        <div className="text-sm font-medium text-telegram-text truncate">{file.name}</div>
                        <div className="text-xs text-telegram-subtext mt-0.5">{file.sizeStr}</div>
                    </div>

                    {!shareInfo ? (
                        <>
                            {/* Security Option */}
                            <div className="space-y-2">
                                <div className="flex items-center justify-between py-1">
                                    <span className="text-sm font-medium text-telegram-text flex items-center gap-2 select-none">
                                        <Shield className="w-4 h-4 text-emerald-400" />
                                        Password Protection
                                    </span>
                                    <button
                                        type="button"
                                        onClick={() => setRequirePassword(!requirePassword)}
                                        className={`relative w-10 h-5.5 rounded-full transition-colors duration-200 shrink-0 ${
                                            requirePassword ? 'bg-telegram-primary' : 'bg-telegram-border'
                                        }`}
                                    >
                                        <span
                                            className={`absolute top-0.5 left-0.5 w-4.5 h-4.5 rounded-full bg-white shadow transition-transform duration-200 ${
                                                requirePassword ? 'translate-x-4.5' : 'translate-x-0'
                                            }`}
                                        />
                                    </button>
                                </div>
                                
                                <AnimatePresence>
                                    {requirePassword && (
                                        <motion.div
                                            initial={{ height: 0, opacity: 0, marginTop: 0 }}
                                            animate={{ height: 'auto', opacity: 1, marginTop: 8 }}
                                            exit={{ height: 0, opacity: 0, marginTop: 0 }}
                                            transition={{ duration: 0.2, ease: 'easeInOut' }}
                                            className="overflow-hidden"
                                        >
                                            <input
                                                type="password"
                                                placeholder="Enter link password"
                                                value={password}
                                                onChange={(e) => setPassword(e.target.value)}
                                                className="w-full bg-telegram-surface/50 border border-telegram-border rounded-lg px-3 py-2 text-sm text-telegram-text focus:outline-none focus:border-telegram-primary placeholder:text-telegram-subtext/60"
                                                autoFocus
                                            />
                                        </motion.div>
                                    )}
                                </AnimatePresence>
                            </div>

                            {/* Expiry Option */}
                            <div className="space-y-2">
                                <span className="text-sm font-medium text-telegram-text flex items-center gap-2">
                                    <Clock className="w-4 h-4 text-amber-400" />
                                    Expiration
                                </span>
                                <div className="grid grid-cols-3 gap-2">
                                    {(['1h', '1d', '7d'] as const).map((type) => (
                                        <button
                                            key={type}
                                            type="button"
                                            onClick={() => setExpiryType(type)}
                                            className={`px-3 py-1.5 rounded-lg text-xs font-medium border transition-colors ${
                                                expiryType === type 
                                                    ? 'bg-telegram-primary border-telegram-primary text-white' 
                                                    : 'bg-telegram-surface border-telegram-border text-telegram-text hover:bg-telegram-hover'
                                            }`}
                                        >
                                            {type === '1h' ? '1 Hour' : type === '1d' ? '1 Day' : '7 Days'}
                                        </button>
                                    ))}
                                    <button
                                        type="button"
                                        onClick={() => setExpiryType('never')}
                                        className={`px-3 py-1.5 rounded-lg text-xs font-medium border transition-colors ${
                                            expiryType === 'never' 
                                                ? 'bg-telegram-primary border-telegram-primary text-white' 
                                                : 'bg-telegram-surface border-telegram-border text-telegram-text hover:bg-telegram-hover'
                                        }`}
                                    >
                                        Never
                                    </button>
                                    <button
                                        type="button"
                                        onClick={() => setExpiryType('custom')}
                                        className={`col-span-2 px-3 py-1.5 rounded-lg text-xs font-medium border transition-colors ${
                                            expiryType === 'custom' 
                                                ? 'bg-telegram-primary border-telegram-primary text-white' 
                                                : 'bg-telegram-surface border-telegram-border text-telegram-text hover:bg-telegram-hover'
                                        }`}
                                    >
                                        Custom Hours
                                    </button>
                                </div>

                                {expiryType === 'custom' && (
                                    <div className="flex gap-2 items-center mt-2 animate-in slide-in-from-top-1 duration-100">
                                        <input
                                            type="number"
                                            min="1"
                                            value={customHours}
                                            onChange={(e) => setCustomHours(e.target.value)}
                                            className="w-24 bg-telegram-surface/50 border border-telegram-border rounded-lg px-3 py-2 text-sm text-telegram-text focus:outline-none focus:border-telegram-primary"
                                        />
                                        <span className="text-xs text-telegram-subtext">hours from now</span>
                                    </div>
                                )}
                            </div>

                            {error && (
                                <div className="bg-red-500/10 border border-red-500/20 text-red-400 text-xs rounded-lg p-3 flex gap-2 items-start">
                                    <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
                                    <span>{error}</span>
                                </div>
                            )}

                            <button
                                onClick={handleGenerate}
                                disabled={loading}
                                className="w-full bg-telegram-primary hover:bg-telegram-primary-hover text-white text-sm font-medium py-2.5 rounded-lg shadow-lg hover:shadow-telegram-primary/20 transition-all flex items-center justify-center gap-2 mt-4"
                            >
                                {loading ? (
                                    <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
                                ) : 'Generate Shareable Link'}
                            </button>
                        </>
                    ) : (
                        <div className="space-y-4 animate-in fade-in duration-200">
                            <div className="bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 text-xs rounded-lg p-3 flex gap-2 items-center">
                                <Check className="w-4 h-4 shrink-0" />
                                <span>Link created successfully!</span>
                            </div>

                            {/* Shareable Link Display */}
                            <div className="space-y-1.5">
                                <label className="text-xs font-semibold text-telegram-subtext">Share Link</label>
                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        readOnly
                                        value={getDisplayLink()}
                                        className="flex-1 bg-telegram-surface/50 border border-telegram-border rounded-lg px-3 py-2 text-sm text-telegram-text focus:outline-none select-all"
                                    />
                                    <button
                                        onClick={handleCopy}
                                        className={`px-3 py-2 rounded-lg border flex items-center justify-center transition-all ${
                                            copied 
                                                ? 'bg-emerald-500 border-emerald-500 text-white' 
                                                : 'bg-telegram-hover border-telegram-border text-telegram-text hover:bg-white/10'
                                        }`}
                                    >
                                        {copied ? <Check className="w-4 h-4" /> : <Copy className="w-4 h-4" />}
                                    </button>
                                </div>
                            </div>

                            {/* Tailscale / Network Share Customizer */}
                            <div className="bg-telegram-hover/30 border border-telegram-border/50 rounded-lg p-3 space-y-2">
                                <div className="text-xs font-semibold text-telegram-text flex items-center gap-1.5">
                                    <span>🌐</span> Share Externally (Tailscale / LAN)
                                </div>
                                <p className="text-xs text-telegram-subtext leading-relaxed">
                                    To share with someone else on your Tailscale network or local WiFi, enter your machine's IP address or hostname below:
                                </p>
                                <div className="flex gap-2 items-center">
                                    <input
                                        type="text"
                                        placeholder="e.g. 100.115.22.45 or tailscale-pc:14201"
                                        value={customDomain}
                                        onChange={(e) => setCustomDomain(e.target.value)}
                                        className="flex-1 bg-telegram-surface/50 border border-telegram-border rounded-lg px-3 py-1.5 text-xs text-telegram-text focus:outline-none focus:border-telegram-primary placeholder:text-telegram-subtext/40"
                                    />
                                </div>
                            </div>

                            <button
                                onClick={onClose}
                                className="w-full bg-telegram-hover hover:bg-white/10 text-telegram-text text-sm font-medium py-2 rounded-lg transition-colors border border-telegram-border"
                            >
                                Done
                            </button>
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
}
