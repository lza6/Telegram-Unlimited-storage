import { QRCodeSVG } from "qrcode.react";

interface QRLoginProps {
    loading: boolean;
    qrUrl: string | null;
    qrPolling: boolean;
    onQrLogin: () => void;
    onSwitchToSetup: () => void;
}

export function QRLogin({
    loading,
    qrUrl,
    qrPolling,
    onQrLogin,
    onSwitchToSetup,
}: QRLoginProps) {
    return (
        <div className="flex flex-col items-center gap-5" aria-label="QR code login">
            {loading && !qrUrl && (
                <div className="w-52 h-52 rounded-2xl bg-white/5 flex items-center justify-center">
                    <div className="w-8 h-8 border-2 border-blue-400 border-t-transparent rounded-full animate-spin" />
                </div>
            )}
            {qrUrl && (
                <>
                    <div className="p-4 bg-white rounded-2xl shadow-xl">
                        <QRCodeSVG
                            value={qrUrl}
                            size={200}
                            level="M"
                            bgColor="#ffffff"
                            fgColor="#000000"
                        />
                    </div>
                    <div className="text-center space-y-1">
                        <p className="text-sm text-white/80">Scan with your Telegram app</p>
                        <p className="text-xs text-white/40">Settings &gt; Devices &gt; Link Desktop Device</p>
                    </div>
                    {qrPolling && (
                        <div className="flex items-center gap-2 text-xs text-blue-300">
                            <div className="w-3 h-3 border-2 border-blue-400 border-t-transparent rounded-full animate-spin" />
                            Waiting for scan...
                        </div>
                    )}
                    <button
                        type="button"
                        onClick={onQrLogin}
                        className="text-xs text-white/50 hover:text-white transition-colors"
                    >
                        Refresh QR Code
                    </button>
                </>
            )}
            <button type="button" onClick={onSwitchToSetup} className="text-xs text-gray-500 hover:text-white transition-colors py-2">
                Back to Configuration
            </button>
        </div>
    );
}
