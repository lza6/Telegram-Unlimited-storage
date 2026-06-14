import { motion } from "framer-motion";

interface FloodWaitScreenProps {
    floodWait: number;
}

export function FloodWaitScreen({ floodWait }: FloodWaitScreenProps) {
    return (
        <motion.div
            key="flood"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            className="text-center space-y-6"
            role="alert"
            aria-live="polite"
            aria-label="Rate limit countdown"
        >
            <div className="w-16 h-16 bg-red-500/20 rounded-full flex items-center justify-center mx-auto animate-pulse">
                <span className="text-2xl">⏳</span>
            </div>
            <div>
                <h2 className="text-xl font-bold text-white mb-2">Too Many Requests</h2>
                <p className="text-sm text-gray-400">Telegram has temporarily limited your actions.</p>
                <p className="text-sm text-gray-400">Please wait before trying again.</p>
            </div>

            <div className="text-5xl font-mono items-center justify-center flex text-blue-400 font-bold">
                {Math.floor(floodWait / 60)}:{(floodWait % 60).toString().padStart(2, '0')}
            </div>

            <p className="text-xs text-red-400/60 mt-4">
                Do not restart the app. The timer will reset if you do.
            </p>
        </motion.div>
    );
}
