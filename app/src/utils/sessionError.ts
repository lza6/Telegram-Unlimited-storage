/** True when a transfer/API error likely means the Telegram session is dead. */
export function isSessionLostError(error: string): boolean {
    const lower = error.toLowerCase();
    const sessionSpecific = [
        'not connected',
        'auto-reconnect failed',
        'no client',
        'telegram client is not connected',
        'session expired',
        'session lost',
        'session revoked',
        'auth key',
        'get_me',
        'please log in',
        'sign in again',
        'unauthorized',
        'user deactivated',
        'user banned',
    ];
    if (sessionSpecific.some((k) => lower.includes(k))) {
        return true;
    }
    // Avoid forcing logout on transient network blips (timeout/econnrefused alone).
    if (lower.includes('disconnect') && !lower.includes('network')) {
        return true;
    }
    return false;
}