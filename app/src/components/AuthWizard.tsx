import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import { Heart } from "lucide-react";
import { load } from '@tauri-apps/plugin-store';
import {
    AuthThemeToggle,
    BrowserFallback,
    FloodWaitScreen,
    SetupStep,
    PhoneStep,
    CodeStep,
    PasswordStep,
    HelpModal,
    DonateModal,
} from './auth';
import type { Step } from './auth';

export function AuthWizard({ onLogin }: { onLogin: () => void }) {
    const isBrowser = typeof window !== 'undefined' && !('__TAURI_INTERNALS__' in window);

    if (isBrowser) {
        return <BrowserFallback />;
    }

    const [step, setStep] = useState<Step>("setup");
    const [loading, setLoading] = useState(false);

    const [apiId, setApiId] = useState("");
    const [apiHash, setApiHash] = useState("");

    const [phone, setPhone] = useState("");
    const [code, setCode] = useState("");
    const [password, setPassword] = useState("");
    const [error, setError] = useState<string | null>(null);
    const [floodWait, setFloodWait] = useState<number | null>(null);
    const [showHelp, setShowHelp] = useState(false);
    const [showDonate, setShowDonate] = useState(false);
    const [loginMethod, setLoginMethod] = useState<'phone' | 'qr'>('phone');
    const [qrUrl, setQrUrl] = useState<string | null>(null);
    const [qrPolling, setQrPolling] = useState(false);
    const qrPollRef = useRef<ReturnType<typeof setInterval> | null>(null);


    useEffect(() => {
        if (!floodWait) return;
        const interval = setInterval(() => {
            setFloodWait(prev => {
                if (prev === null || prev <= 1) return null;
                return prev - 1;
            });
        }, 1000);
        return () => clearInterval(interval);
    }, [floodWait]);

    useEffect(() => {
        const initStore = async () => {
            try {
                const store = await load('config.json');
                const savedId = await store.get<string>('api_id');
                const savedHash = await store.get<string>('api_hash');

                if (savedId && savedHash) {
                    setApiId(savedId);
                    setApiHash(savedHash);
                }
            } catch {
                // config not found, starting fresh
            }
        };
        initStore();
    }, []);

    const saveCredentials = async () => {
        try {
            const store = await load('config.json');
            await store.set('api_id', apiId);
            await store.set('api_hash', apiHash);
            await store.save();
        } catch {
            // store write failure, non-critical
        }
    };

    const handleSetupSubmit = async (e: React.FormEvent) => {
        e.preventDefault();

        if (apiId.includes(' ') || apiHash.includes(' ')) {
            setError("API ID and API Hash cannot contain spaces. Please remove any spaces.");
            return;
        }

        if (!apiId || !apiHash) {
            setError("Both API ID and Hash are required.");
            return;
        }
        setError(null);
        await saveCredentials();
        setStep("phone");
        setLoginMethod('phone');
        setQrUrl(null);
        setQrPolling(false);
    };

    const handleQrLogin = async () => {
        setError(null);
        setLoading(true);
        try {
            const idInt = parseInt(apiId, 10);
            if (isNaN(idInt)) throw new Error("API ID must be a number");

            const url = await invoke<string>("cmd_auth_qr_login", {
                apiId: idInt,
                apiHash: apiHash
            });

            if (url === "__authorized__") {
                onLogin();
                return;
            }

            setQrUrl(url);
            setQrPolling(true);
        } catch (err: unknown) {
            setError(err instanceof Error ? err.message : String(err));
        } finally {
            setLoading(false);
        }
    };

    // QR polling effect
    useEffect(() => {
        if (!qrPolling) {
            if (qrPollRef.current) {
                clearInterval(qrPollRef.current);
                qrPollRef.current = null;
            }
            return;
        }

        qrPollRef.current = setInterval(async () => {
            try {
                const res = await invoke<{ success: boolean; next_step?: string }>("cmd_auth_qr_poll");
                if (res.success) {
                    setQrPolling(false);
                    if (res.next_step === "password") {
                        setStep("password");
                    } else {
                        onLogin();
                    }
                }
                // If next_step === "waiting", keep polling
            } catch {
                // Polling error — keep trying silently
            }
        }, 3000);

        return () => {
            if (qrPollRef.current) {
                clearInterval(qrPollRef.current);
                qrPollRef.current = null;
            }
        };
    }, [qrPolling, apiId, apiHash]);

    const handlePhoneSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setLoading(true);
        setError(null);
        try {
            const idInt = parseInt(apiId, 10);
            if (isNaN(idInt)) throw new Error("API ID must be a number");

            await invoke("cmd_auth_request_code", {
                phone,
                apiId: idInt,
                apiHash: apiHash
            });
            setStep("code");
        } catch (err: unknown) {
            const msg = err instanceof Error ? err.message : JSON.stringify(err);
            if (msg.includes("FLOOD_WAIT_")) {
                const parts = msg.split("FLOOD_WAIT_");
                if (parts[1]) {
                    const seconds = parseInt(parts[1]);
                    if (!isNaN(seconds)) {
                        setFloodWait(seconds);
                        return;
                    }
                }
            }
            setError(msg);
        } finally {
            setLoading(false);
        }
    };

    const handleCodeSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setLoading(true);
        setError(null);
        try {
            const res = await invoke<{ success: boolean; next_step?: string }>("cmd_auth_sign_in", { code });
            if (res.success) {
                onLogin();
            } else if (res.next_step === "password") {
                setStep("password");
            } else {
                setError("Unknown error");
            }
        } catch (err: unknown) {
            setError(err instanceof Error ? err.message : String(err));
        } finally {
            setLoading(false);
        }
    };

    const handlePasswordSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        setLoading(true);
        setError(null);
        try {
            const res = await invoke<{ success: boolean; next_step?: string }>("cmd_auth_check_password", { password });
            if (res.success) {
                onLogin();
            } else {
                setError("Password verification failed.");
            }
        } catch (err: unknown) {
            setError(err instanceof Error ? err.message : String(err));
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="h-full w-full auth-gradient flex items-center justify-center p-6 relative">
            <AuthThemeToggle />

            <motion.div
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                className="auth-glass p-8 rounded-3xl shadow-2xl w-full max-w-md"
            >
                <div className="text-center mb-8">
                    <div className="w-20 h-20 mb-6 mx-auto flex items-center justify-center filter drop-shadow-lg">
                        <img src="/logo.svg" alt="Logo" className="w-full h-full" />
                    </div>
                    <h1 className="text-2xl font-bold text-white mb-1 tracking-tight">Telegram Drive</h1>
                    <p className="text-sm text-white/60 font-medium">Self-Hosted Secure Storage</p>
                </div>

                <AnimatePresence mode="wait">
                    {floodWait ? (
                        <FloodWaitScreen floodWait={floodWait} />
                    ) : (
                        <>
                            {step === "setup" && (
                                <SetupStep
                                    apiId={apiId}
                                    apiHash={apiHash}
                                    onApiIdChange={setApiId}
                                    onApiHashChange={setApiHash}
                                    onSubmit={handleSetupSubmit}
                                    onShowHelp={() => setShowHelp(true)}
                                    onDevLogin={import.meta.env.DEV ? onLogin : undefined}
                                />
                            )}

                            {step === "phone" && (
                                <PhoneStep
                                    phone={phone}
                                    loading={loading}
                                    loginMethod={loginMethod}
                                    qrUrl={qrUrl}
                                    qrPolling={qrPolling}
                                    onPhoneChange={setPhone}
                                    onPhoneSubmit={handlePhoneSubmit}
                                    onSwitchToSetup={() => setStep("setup")}
                                    onSwitchMethod={(method) => {
                                        setLoginMethod(method);
                                        setError(null);
                                        if (method === 'phone') {
                                            setQrUrl(null);
                                            setQrPolling(false);
                                        } else {
                                            handleQrLogin();
                                        }
                                    }}
                                    onQrLogin={handleQrLogin}
                                    onQrPollStop={() => setQrPolling(false)}
                                />
                            )}

                            {step === "code" && (
                                <CodeStep
                                    code={code}
                                    loading={loading}
                                    onCodeChange={setCode}
                                    onSubmit={handleCodeSubmit}
                                    onBackToPhone={() => setStep("phone")}
                                />
                            )}

                            {step === "password" && (
                                <PasswordStep
                                    password={password}
                                    loading={loading}
                                    onPasswordChange={setPassword}
                                    onSubmit={handlePasswordSubmit}
                                    onBackToCode={() => { setStep("code"); setPassword(""); setError(null); }}
                                />
                            )}
                        </>
                    )}
                </AnimatePresence>

                {error && (
                    <motion.div
                        initial={{ opacity: 0, y: 10 }}
                        animate={{ opacity: 1, y: 0 }}
                        className="mt-6 p-4 bg-red-500/10 border border-red-500/20 rounded-xl flex items-start gap-3"
                        role="alert"
                        aria-live="assertive"
                    >
                        <div className="w-1.5 h-1.5 rounded-full bg-red-500 mt-2 shrink-0" />
                        <p className="text-red-400 text-sm leading-snug">{error}</p>
                    </motion.div>
                )}

                <div className="mt-8 pt-4 border-t border-white/5 text-center">
                    <button
                        onClick={() => setShowDonate(true)}
                        className="text-xs text-telegram-subtext hover:text-telegram-text transition-colors flex items-center justify-center gap-1.5 mx-auto"
                    >
                        <Heart className="w-3.5 h-3.5 text-red-500/80" />
                        Donate
                    </button>
                </div>
            </motion.div>

            <AnimatePresence>
                {showHelp && (
                    <HelpModal onClose={() => setShowHelp(false)} />
                )}
            </AnimatePresence>

            <AnimatePresence>
                {showDonate && (
                    <DonateModal onClose={() => setShowDonate(false)} />
                )}
            </AnimatePresence>

            <div className="fixed top-[-20%] left-[-10%] w-[500px] h-[500px] bg-blue-600/20 rounded-full blur-[120px] pointer-events-none -z-10" />
            <div className="fixed bottom-[-10%] right-[-10%] w-[400px] h-[400px] bg-purple-600/10 rounded-full blur-[100px] pointer-events-none -z-10" />
        </div>
    );
}
