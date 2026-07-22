import { useCallback } from 'react';
import { toast } from 'sonner';
import { canTransferFiles, connectionStatusLabel, type ConnectionStatus } from '../types/connection';

/**
 * Shared hook for guarding file transfer operations.
 * Checks if transfers are allowed and shows appropriate error toast if not.
 *
 * @param connectionStatus - Current connection status
 * @param opts - Optional custom blocked message
 * @returns guardTransfer function that returns true if transfer is allowed
 */
export function useGuardTransfer(
    connectionStatus: ConnectionStatus,
    opts?: {
        customBlockedMessage?: string;
    },
): () => boolean {
    const guardTransfer = useCallback((): boolean => {
        if (!canTransferFiles(connectionStatus)) {
            toast.error(opts?.customBlockedMessage || connectionStatusLabel(connectionStatus));
            return false;
        }
        return true;
    }, [connectionStatus, opts?.customBlockedMessage]);

    return guardTransfer;
}

/**
 * Simplified guard for hooks that receive canTransfer and blockedMessage as options.
 * Used by useFileUpload, useFileDownload, useFileOperations.
 */
export function guardTransferWithOpts(
    canTransfer: (() => boolean) | undefined,
    blockedMessage: string | undefined,
): boolean {
    if (canTransfer && !canTransfer()) {
        toast.error(blockedMessage || 'Telegram 会话未就绪');
        return false;
    }
    return true;
}
