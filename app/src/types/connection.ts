export type ConnectionStatus = 'checking' | 'online' | 'session_lost' | 'network_offline';

export function connectionStatusLabel(status: ConnectionStatus): string {
    switch (status) {
        case 'checking':
            return 'Checking Telegram connection — verifying session…';
        case 'online':
            return 'Telegram session active';
        case 'session_lost':
            return 'Session expired — sign in again';
        case 'network_offline':
            return 'No network connection';
    }
}

export function canTransferFiles(status: ConnectionStatus): boolean {
    return status === 'online';
}

/** Bot / headless REST ready — browse index without GramJS User session. */
export function isServiceReady(opts: {
    connectionStatus: ConnectionStatus;
    apiHealthReady?: boolean;
}): boolean {
    if (opts.apiHealthReady === true) return true;
    return canTransferFiles(opts.connectionStatus);
}

export function isBotTransportMode(transportMode: string | null | undefined): boolean {
    return (transportMode || '').toLowerCase() === 'bot';
}

/** Bot index mode: list/delete index entries; uploads still need User session. */
export function isBotIndexReady(opts: {
    apiHealthReady?: boolean;
    transportMode?: string | null;
}): boolean {
    return opts.apiHealthReady === true && isBotTransportMode(opts.transportMode);
}

/** Download via GramJS or Bot index REST stream. */
export function canDownloadFiles(opts: {
    transferReady: boolean;
    botIndexReady?: boolean;
}): boolean {
    if (opts.transferReady) return true;
    return opts.botIndexReady === true;
}

/** Preview / stream / thumbnail — same transport gate as download in Bot mode. */
export function canPreviewFiles(opts: {
    transferReady: boolean;
    botIndexReady?: boolean;
}): boolean {
    return canDownloadFiles(opts);
}

/** Share link creation — DB-only; same gate as download in Bot mode. */
export function canShareFiles(opts: {
    transferReady: boolean;
    botIndexReady?: boolean;
}): boolean {
    return canDownloadFiles(opts);
}

/** Map network + Telegram probe to sidebar connection status. */
export function classifyConnectionStatus(
    networkOnline: boolean,
    telegramOk: boolean,
): ConnectionStatus {
    if (!networkOnline) return 'network_offline';
    return telegramOk ? 'online' : 'session_lost';
}
