import { ShieldCheck } from "lucide-react";

export function BrowserFallback() {
    return (
        <div className="flex flex-col items-center justify-center h-full max-w-lg mx-auto p-8 text-center">
            <div className="w-20 h-20 bg-red-500/20 rounded-full flex items-center justify-center mb-6">
                <ShieldCheck className="w-10 h-10 text-red-500" />
            </div>
            <h1 className="text-2xl font-bold text-white mb-4">Desktop App Required</h1>
            <p className="text-gray-400 mb-6 leading-relaxed">
                You are viewing the internal development server in a browser.
                This application cannot function here because it requires access to the system backend (Rust).
            </p>
            <div className="p-4 bg-gray-800 rounded-xl border border-gray-700 text-sm text-gray-300">
                Please open the <strong>Telegram Drive</strong> window in your OS taskbar/dock to continue.
            </div>
        </div>
    );
}
