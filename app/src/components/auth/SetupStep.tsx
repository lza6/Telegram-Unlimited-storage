import { motion } from "framer-motion";
import { Key, Settings, HelpCircle } from "lucide-react";

interface SetupStepProps {
    apiId: string;
    apiHash: string;
    onApiIdChange: (value: string) => void;
    onApiHashChange: (value: string) => void;
    onSubmit: (e: React.FormEvent) => void;
    onShowHelp: () => void;
    onDevLogin?: () => void;
}

export function SetupStep({
    apiId,
    apiHash,
    onApiIdChange,
    onApiHashChange,
    onSubmit,
    onShowHelp,
    onDevLogin,
}: SetupStepProps) {
    return (
        <motion.form
            key="setup"
            initial={{ x: 20, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: -20, opacity: 0 }}
            onSubmit={onSubmit}
            className="space-y-5"
            aria-label="API credentials setup form"
        >
            <div className="space-y-4">
                <div>
                    <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2">API ID</label>
                    <div className="relative">
                        <Key className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 auth-form-icon" />
                        <input
                            type="text"
                            value={apiId}
                            onChange={(e) => onApiIdChange(e.target.value)}
                            placeholder="12345678"
                            className="w-full glass-input rounded-xl pl-12 pr-4 py-3.5 text-white placeholder-gray-600 focus:outline-none focus:border-blue-500 transition-all font-mono text-sm"
                            aria-label="API ID"
                        />
                    </div>
                </div>
                <div>
                    <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2">API Hash</label>
                    <div className="relative">
                        <Key className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 auth-form-icon" />
                        <input
                            type="text"
                            value={apiHash}
                            onChange={(e) => onApiHashChange(e.target.value)}
                            placeholder="abcdef123456..."
                            className="w-full glass-input rounded-xl pl-12 pr-4 py-3.5 text-white placeholder-gray-600 focus:outline-none focus:border-blue-500 transition-all font-mono text-sm"
                            aria-label="API Hash"
                        />
                    </div>
                </div>
            </div>

            <button
                type="submit"
                className="w-full bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-400 text-white font-bold py-4 rounded-xl flex items-center justify-center gap-2 transition-all shadow-lg shadow-blue-900/20 active:scale-[0.98]"
            >
                Configure <Settings className="w-4 h-4" />
            </button>

            <button
                type="button"
                onClick={onShowHelp}
                className="w-full text-xs text-blue-300 hover:text-white transition-colors flex items-center justify-center gap-1.5 py-1"
            >
                <HelpCircle className="w-3 h-3" />
                How do I get my API credentials?
            </button>

            {onDevLogin && (
                <button
                    type="button"
                    onClick={onDevLogin}
                    className="w-full text-xs text-red-400/60 hover:text-red-300 transition-colors py-1"
                >
                    Dev Mode
                </button>
            )}
        </motion.form>
    );
}
