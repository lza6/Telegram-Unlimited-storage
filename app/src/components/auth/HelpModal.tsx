import { motion } from "framer-motion";
import { X, ExternalLink } from "lucide-react";
import { open } from '@tauri-apps/plugin-shell';

interface HelpModalProps {
    onClose: () => void;
}

export function HelpModal({ onClose }: HelpModalProps) {
    return (
        <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4"
            onClick={onClose}
            role="dialog"
            aria-modal="true"
            aria-label="Help: Getting Started"
        >
            <motion.div
                initial={{ scale: 0.95, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0.95, opacity: 0 }}
                className="glass bg-telegram-surface border border-telegram-border rounded-2xl p-6 max-w-lg w-full max-h-[80vh] overflow-y-auto shadow-2xl"
                onClick={(e) => e.stopPropagation()}
            >
                <div className="flex items-center justify-between mb-6">
                    <h2 className="text-xl font-bold text-telegram-text">Getting Started</h2>
                    <button onClick={onClose} className="p-2 hover:bg-telegram-hover rounded-lg transition-colors" aria-label="Close help">
                        <X className="w-5 h-5 text-telegram-subtext" />
                    </button>
                </div>

                <div className="space-y-6 text-telegram-text">
                    <div className="p-4 bg-telegram-primary/10 border border-telegram-primary/20 rounded-xl">
                        <p className="text-sm text-telegram-subtext">
                            <strong className="text-telegram-primary">Telegram Drive</strong> uses your Telegram account as secure cloud storage. You'll need a Telegram account and API credentials to get started.
                        </p>
                    </div>

                    <div className="space-y-4">
                        <h3 className="font-semibold flex items-center gap-2">
                            <span className="w-6 h-6 bg-telegram-primary text-white text-xs font-bold rounded-full flex items-center justify-center">1</span>
                            Go to Telegram's Developer Portal
                        </h3>
                        <p className="text-sm text-telegram-subtext ml-8">
                            Visit <button type="button" onClick={(e) => { e.preventDefault(); open('https://my.telegram.org'); }} className="text-telegram-primary underline hover:text-telegram-text cursor-pointer">my.telegram.org</button> and log in with your phone number.
                        </p>
                    </div>

                    <div className="space-y-4">
                        <h3 className="font-semibold flex items-center gap-2">
                            <span className="w-6 h-6 bg-telegram-primary text-white text-xs font-bold rounded-full flex items-center justify-center">2</span>
                            Create a New Application
                        </h3>
                        <p className="text-sm text-telegram-subtext ml-8">
                            Click on <strong>"API development tools"</strong> and create a new application. Use any name and description you like.
                        </p>
                    </div>

                    <div className="space-y-4">
                        <h3 className="font-semibold flex items-center gap-2">
                            <span className="w-6 h-6 bg-telegram-primary text-white text-xs font-bold rounded-full flex items-center justify-center">3</span>
                            Copy Your Credentials
                        </h3>
                        <p className="text-sm text-telegram-subtext ml-8">
                            After creating the app, you'll see your <strong>API ID</strong> (a number) and <strong>API Hash</strong> (a string). Copy both and paste them into the fields on the previous screen.
                        </p>
                    </div>

                    <div className="p-4 bg-telegram-hover rounded-xl border border-telegram-border">
                        <p className="text-xs text-telegram-subtext">
                            <strong>Privacy:</strong> Your credentials are stored locally on your device and are never sent to any third-party servers. All data goes directly between you and Telegram.
                        </p>
                    </div>

                    <button
                        type="button"
                        onClick={(e) => { e.preventDefault(); open('https://my.telegram.org'); }}
                        className="w-full bg-telegram-primary text-white font-semibold py-3 rounded-xl flex items-center justify-center gap-2 hover:bg-telegram-primary/90 transition-colors"
                    >
                        <ExternalLink className="w-4 h-4" />
                        Open my.telegram.org
                    </button>
                </div>
            </motion.div>
        </motion.div>
    );
}
