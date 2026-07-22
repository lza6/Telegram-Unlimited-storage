import { motion } from "framer-motion";
import { X } from "lucide-react";
import { open } from '@tauri-apps/plugin-shell';

interface DonateModalProps {
    onClose: () => void;
}

export function DonateModal({ onClose }: DonateModalProps) {
    return (
        <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 bg-black/70 backdrop-blur-sm flex items-center justify-center p-4"
            onClick={onClose}
            role="dialog"
            aria-modal="true"
            aria-label="Support the Project"
        >
            <motion.div
                initial={{ scale: 0.95, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                exit={{ scale: 0.95, opacity: 0 }}
                className="glass bg-telegram-surface border border-telegram-border rounded-2xl p-6 max-w-sm w-full shadow-2xl"
                onClick={(e) => e.stopPropagation()}
            >
                <div className="relative flex items-center justify-center mb-6">
                    <h2 className="text-xl font-bold text-telegram-text text-center">
                        Support the Project
                    </h2>
                    <button onClick={onClose} className="absolute right-0 p-2 hover:bg-telegram-hover rounded-lg transition-colors" aria-label="Close donate modal">
                        <X className="w-5 h-5 text-telegram-subtext" />
                    </button>
                </div>

                <div className="space-y-4 text-center">
                    <p className="text-sm text-telegram-subtext mb-6">
                        If you find Telegram Drive useful, consider supporting its development!
                    </p>

                    <div className="space-y-4">
                        <a href="#" onClick={(e) => { e.preventDefault(); open('https://www.paypal.me/Caamer20'); }} className="block hover:opacity-80 transition-opacity">
                            <img src="https://raw.githubusercontent.com/stefan-niedermann/paypal-donate-button/master/paypal-donate-button.png" alt="Donate with PayPal" width="200" className="mx-auto" />
                        </a>

                        <a href="#" onClick={(e) => { e.preventDefault(); open('https://link.trustwallet.com/send?address=ltc1q6wkr5ac4u0pxx4hx7xgwn0gsaku25ws0df73rp&asset=c2'); }} className="block hover:opacity-80 transition-opacity">
                            <img src="https://img.shields.io/badge/Donate-LTC-345D9D?style=for-the-badge&logo=litecoin&logoColor=white" alt="Donate LTC" className="mx-auto h-[28px]" />
                        </a>

                        <a href="#" onClick={(e) => { e.preventDefault(); open('https://link.trustwallet.com/send?asset=c0&address=bc1q5pt7m2fk6w0dzsnf6vvd5k6nw5k44785286ujy'); }} className="block hover:opacity-80 transition-opacity">
                            <img src="https://img.shields.io/badge/Donate-BTC-F7931A?style=for-the-badge&logo=bitcoin&logoColor=white" alt="Donate BTC" className="mx-auto h-[28px]" />
                        </a>
                    </div>
                </div>
            </motion.div>
        </motion.div>
    );
}
