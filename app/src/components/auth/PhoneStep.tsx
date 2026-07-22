import { motion } from "framer-motion";
import { Phone, ArrowRight, QrCode } from "lucide-react";
import { QRLogin } from "./QRLogin";

interface PhoneStepProps {
    phone: string;
    loading: boolean;
    loginMethod: "phone" | "qr";
    qrUrl: string | null;
    qrPolling: boolean;
    onPhoneChange: (value: string) => void;
    onPhoneSubmit: (e: React.FormEvent) => void;
    onSwitchToSetup: () => void;
    onSwitchMethod: (method: "phone" | "qr") => void;
    onQrLogin: () => void;
    onQrPollStop: () => void;
}

export function PhoneStep({
    phone,
    loading,
    loginMethod,
    qrUrl,
    qrPolling,
    onPhoneChange,
    onPhoneSubmit,
    onSwitchToSetup,
    onSwitchMethod,
    onQrLogin,
    onQrPollStop,
}: PhoneStepProps) {
    return (
        <motion.div
            key="phone"
            initial={{ x: 20, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: -20, opacity: 0 }}
            className="space-y-6"
        >
            {/* Phone / QR Toggle */}
            <div className="flex rounded-xl overflow-hidden border border-white/10" role="tablist" aria-label="Login method">
                <button
                    type="button"
                    role="tab"
                    aria-selected={loginMethod === "phone"}
                    onClick={() => onSwitchMethod("phone")}
                    className={`flex-1 py-2.5 text-sm font-medium flex items-center justify-center gap-2 transition-all ${
                        loginMethod === "phone"
                            ? "bg-white/15 text-white"
                            : "text-white/50 hover:text-white/70"
                    }`}
                >
                    <Phone className="w-4 h-4" /> Phone Number
                </button>
                <button
                    type="button"
                    role="tab"
                    aria-selected={loginMethod === "qr"}
                    onClick={() => onSwitchMethod("qr")}
                    className={`flex-1 py-2.5 text-sm font-medium flex items-center justify-center gap-2 transition-all ${
                        loginMethod === "qr"
                            ? "bg-white/15 text-white"
                            : "text-white/50 hover:text-white/70"
                    }`}
                >
                    <QrCode className="w-4 h-4" /> QR Code
                </button>
            </div>

            {loginMethod === "phone" ? (
                <form onSubmit={onPhoneSubmit} className="space-y-6" aria-label="Phone number login form">
                    <div className="space-y-2">
                        <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider">Phone Number</label>
                        <div className="relative">
                            <Phone className="absolute left-4 top-1/2 -translate-y-1/2 w-5 h-5 auth-form-icon" />
                            <input
                                type="tel"
                                value={phone}
                                onChange={(e) => onPhoneChange(e.target.value)}
                                placeholder="+1 234 567 8900"
                                className="w-full glass-input rounded-xl pl-12 pr-4 py-4 text-white placeholder-gray-600 focus:outline-none focus:border-blue-500 transition-all text-lg tracking-wide"
                                aria-label="Phone number"
                            />
                        </div>
                    </div>

                    <div className="flex flex-col gap-3">
                        <button
                            type="submit"
                            disabled={loading}
                            className="w-full bg-white text-black hover:bg-gray-100 font-bold py-4 rounded-xl flex items-center justify-center gap-2 transition-all shadow-lg active:scale-[0.98] disabled:opacity-50 disabled:cursor-not-allowed"
                        >
                            {loading ? "Connecting..." : <>Continue <ArrowRight className="w-5 h-5" /></>}
                        </button>
                        <button type="button" onClick={onSwitchToSetup} className="text-xs text-gray-500 hover:text-white transition-colors py-2">
                            Back to Configuration
                        </button>
                    </div>
                </form>
            ) : (
                <QRLogin
                    loading={loading}
                    qrUrl={qrUrl}
                    qrPolling={qrPolling}
                    onQrLogin={onQrLogin}
                    onSwitchToSetup={() => {
                        onQrPollStop();
                        onSwitchToSetup();
                    }}
                />
            )}
        </motion.div>
    );
}
