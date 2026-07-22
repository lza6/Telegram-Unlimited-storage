import { motion } from "framer-motion";
import { Lock } from "lucide-react";

interface PasswordStepProps {
    password: string;
    loading: boolean;
    onPasswordChange: (value: string) => void;
    onSubmit: (e: React.FormEvent) => void;
    onBackToCode: () => void;
}

export function PasswordStep({
    password,
    loading,
    onPasswordChange,
    onSubmit,
    onBackToCode,
}: PasswordStepProps) {
    return (
        <motion.form
            key="password"
            initial={{ x: 20, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: -20, opacity: 0 }}
            onSubmit={onSubmit}
            className="space-y-6"
            aria-label="Cloud password form"
        >
            <div className="space-y-2">
                <div className="p-3 bg-blue-500/10 border border-blue-500/20 rounded-xl mb-4">
                    <p className="text-xs text-blue-300 text-center">
                        Your account has Two-Factor Authentication enabled.
                        Please enter your cloud password to continue.
                    </p>
                </div>
                <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider">Cloud Password</label>
                <div className="relative">
                    <Lock className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 auth-form-icon" />
                    <input
                        type="password"
                        value={password}
                        onChange={(e) => onPasswordChange(e.target.value)}
                        placeholder="Enter your password"
                        className="w-full glass-input rounded-xl pl-12 pr-4 py-4 text-white placeholder-gray-600 focus:outline-none focus:border-blue-500 transition-all text-lg"
                        autoFocus
                        aria-label="Cloud password"
                    />
                </div>
            </div>

            <div className="flex flex-col gap-3">
                <button
                    type="submit"
                    disabled={loading || !password}
                    className="w-full bg-white text-black hover:bg-gray-100 font-bold py-4 rounded-xl flex items-center justify-center gap-2 transition-all shadow-lg active:scale-[0.98] disabled:opacity-50 disabled:cursor-not-allowed"
                >
                    {loading ? "Verifying..." : "Unlock"}
                </button>
                <button type="button" onClick={onBackToCode} className="text-xs text-gray-500 hover:text-white transition-colors py-2">
                    Back to Code Entry
                </button>
            </div>
        </motion.form>
    );
}
