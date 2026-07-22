import { motion } from "framer-motion";
import { Key } from "lucide-react";

interface CodeStepProps {
    code: string;
    loading: boolean;
    onCodeChange: (value: string) => void;
    onSubmit: (e: React.FormEvent) => void;
    onBackToPhone: () => void;
}

export function CodeStep({
    code,
    loading,
    onCodeChange,
    onSubmit,
    onBackToPhone,
}: CodeStepProps) {
    return (
        <motion.form
            key="code"
            initial={{ x: 20, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: -20, opacity: 0 }}
            onSubmit={onSubmit}
            className="space-y-6"
            aria-label="Telegram code verification form"
        >
            <div className="space-y-2">
                <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider">Telegram Code</label>
                <div className="relative">
                    <Key className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 auth-form-icon" />
                    <input
                        type="text"
                        value={code}
                        onChange={(e) => onCodeChange(e.target.value)}
                        placeholder="1 2 3 4 5"
                        className="w-full glass-input rounded-xl pl-12 pr-4 py-4 text-white placeholder-gray-600 focus:outline-none focus:border-blue-500 transition-all text-2xl tracking-[0.5em] font-mono text-center"
                        aria-label="Telegram verification code"
                    />
                </div>
            </div>

            <div className="flex flex-col gap-3">
                <button
                    type="submit"
                    disabled={loading}
                    className="w-full bg-white text-black hover:bg-gray-100 font-bold py-4 rounded-xl flex items-center justify-center gap-2 transition-all shadow-lg active:scale-[0.98]"
                >
                    {loading ? "Verifying..." : "Sign In"}
                </button>
                <button type="button" onClick={onBackToPhone} className="text-xs text-gray-500 hover:text-white transition-colors py-2">
                    Change Phone Number
                </button>
            </div>
        </motion.form>
    );
}
