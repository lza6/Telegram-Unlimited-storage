import type { ReactNode } from 'react';
import { X, RotateCcw, AlertCircle } from 'lucide-react';
import { countSuccessTransfers, hasActiveTransfers } from '../../lib/queuePure';
import { formatTransferBytes, formatTransferProgressLabel } from '../../lib/transferUiPure';

export interface TransferQueueRow {
    id: string;
    status: string;
    error?: string;
    progress?: number;
    uploadedBytes?: number;
    totalBytes?: number;
    speedBytesPerSec?: number;
}

export interface TransferQueuePanelProps<T extends TransferQueueRow> {
    items: T[];
    panelClassName: string;
    ariaLabel?: string;
    title: string;
    titlePrefix?: ReactNode;
    titleSuffix?: ReactNode;
    activeStatuses: readonly string[];
    inProgressStatus: string;
    progressBarClassName: string;
    getItemLabel: (item: T) => string;
    getItemTooltip?: (item: T) => string;
    renderStatusIndicator: (item: T) => ReactNode;
    onClearFinished: () => void;
    onCancelAll: () => void;
    onCancelItem: (id: string) => void;
    onRetryItem: (id: string) => void;
}

export function TransferQueuePanel<T extends TransferQueueRow>({
    items,
    panelClassName,
    ariaLabel,
    title,
    titlePrefix,
    titleSuffix,
    activeStatuses,
    inProgressStatus,
    progressBarClassName,
    getItemLabel,
    getItemTooltip,
    renderStatusIndicator,
    onClearFinished,
    onCancelAll,
    onCancelItem,
    onRetryItem,
}: TransferQueuePanelProps<T>) {
    if (items.length === 0) return null;

    const completedCount = countSuccessTransfers(items);
    const showCancelAll = hasActiveTransfers(items, activeStatuses);

    return (
        <div
            role={ariaLabel ? 'region' : undefined}
            aria-label={ariaLabel}
            className={panelClassName}
        >
            <div className="p-3 border-b border-telegram-border bg-telegram-hover flex justify-between items-center">
                <div className="flex items-center gap-2">
                    {titlePrefix}
                    <h4 className="text-sm font-medium text-telegram-text">{title}</h4>
                    {titleSuffix}
                </div>
                <div className="flex gap-2">
                    {showCancelAll && (
                        <button
                            onClick={onCancelAll}
                            className="text-xs text-red-400 hover:text-red-300 transition-colors"
                        >
                            Cancel All
                        </button>
                    )}
                    {completedCount > 0 && (
                        <button
                            onClick={onClearFinished}
                            className="text-xs text-telegram-primary hover:text-telegram-text transition-colors"
                        >
                            Clear Finished
                        </button>
                    )}
                </div>
            </div>
            <div className="max-h-60 overflow-y-auto p-2 space-y-2">
                {items.map((item) => (
                    <div key={item.id} className="flex flex-col gap-1 p-2 bg-telegram-hover rounded">
                        <div className="flex items-center gap-3 text-sm">
                            {renderStatusIndicator(item)}
                            <div
                                className="flex-1 truncate text-telegram-subtext"
                                title={getItemTooltip?.(item) ?? getItemLabel(item)}
                            >
                                {getItemLabel(item)}
                            </div>
                            {item.status === inProgressStatus && (
                                <button
                                    onClick={() => onCancelItem(item.id)}
                                    className="text-gray-400 hover:text-red-400 transition-colors flex-shrink-0"
                                    title="Cancel"
                                >
                                    <X className="w-3.5 h-3.5" />
                                </button>
                            )}
                            {item.status === 'pending' && (
                                <button
                                    onClick={() => onCancelItem(item.id)}
                                    className="text-gray-400 hover:text-red-400 transition-colors flex-shrink-0"
                                    title="Remove"
                                >
                                    <X className="w-3.5 h-3.5" />
                                </button>
                            )}
                            {(item.status === 'error' || item.status === 'cancelled') && (
                                <button
                                    onClick={() => onRetryItem(item.id)}
                                    className="text-gray-400 hover:text-blue-400 transition-colors flex-shrink-0"
                                    title="Retry"
                                >
                                    <RotateCcw className="w-3.5 h-3.5" />
                                </button>
                            )}
                        </div>
                        {item.status === inProgressStatus && (
                            <>
                                <div className="w-full bg-telegram-border h-1 mt-1 rounded-full overflow-hidden">
                                    {item.progress !== undefined ? (
                                        <div
                                            className={`${progressBarClassName} h-full rounded-full transition-all duration-300`}
                                            style={{ width: `${item.progress}%` }}
                                        />
                                    ) : (
                                        <div
                                            className={`${progressBarClassName} h-full w-full animate-progress-indeterminate`}
                                        />
                                    )}
                                </div>
                                <div className="flex justify-between text-[10px] text-telegram-subtext mt-0.5">
                                    <span>
                                        {formatTransferProgressLabel(
                                            item.progress,
                                            item.uploadedBytes,
                                            item.totalBytes,
                                        )}
                                    </span>
                                    <span>
                                        {item.speedBytesPerSec !== undefined && item.speedBytesPerSec > 0
                                            ? `${formatTransferBytes(item.speedBytesPerSec)}/s`
                                            : ''}
                                    </span>
                                </div>
                            </>
                        )}
                        {item.status === 'error' && item.error && (
                            <div className="flex items-center gap-1 text-xs text-red-400 mt-1">
                                <AlertCircle className="w-3 h-3 flex-shrink-0" />
                                <span className="truncate">{item.error}</span>
                            </div>
                        )}
                        {item.status === 'cancelled' && (
                            <div className="text-xs text-gray-400 mt-0.5">Cancelled</div>
                        )}
                    </div>
                ))}
            </div>
        </div>
    );
}
