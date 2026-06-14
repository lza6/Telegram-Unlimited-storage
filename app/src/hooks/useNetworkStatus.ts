import { useState, useEffect, useRef } from 'react';
import { useSettings } from '../context/SettingsContext';

/**
 * Network detection with adaptive polling when VPN optimizer is enabled.
 */
export function useNetworkStatus() {
    const [isOnline, setIsOnline] = useState(true);
    const { settings, isLoaded } = useSettings();
    const onlineRef = useRef(true);

    useEffect(() => {
        if (!isLoaded) return;

        let cancelled = false;
        let timer: ReturnType<typeof setTimeout> | undefined;

        const scheduleNext = (ms: number) => {
            if (cancelled) return;
            timer = setTimeout(runCheck, ms);
        };

        const runCheck = async () => {
            try {
                const { invoke } = await import('@tauri-apps/api/core');
                const available = await invoke<boolean>('cmd_is_network_available');
                if (cancelled) return;
                onlineRef.current = available;
                setIsOnline(available);

                let intervalMs = 10_000;
                if (settings.vpnMode && settings.adaptivePolling) {
                    intervalMs = await invoke<number>('cmd_get_polling_interval_ms', {
                        lastCheckOk: available,
                    });
                }
                scheduleNext(intervalMs);
            } catch {
                if (cancelled) return;
                onlineRef.current = false;
                setIsOnline(false);
                scheduleNext(settings.vpnMode && settings.adaptivePolling
                    ? settings.pollingMaxSec * 1000
                    : 10_000);
            }
        };

        runCheck();
        return () => {
            cancelled = true;
            if (timer) clearTimeout(timer);
        };
    }, [
        isLoaded,
        settings.vpnMode,
        settings.adaptivePolling,
        settings.pollingMinSec,
        settings.pollingMaxSec,
    ]);

    return isOnline;
}
