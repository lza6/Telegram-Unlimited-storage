import { DownloadItem } from '../../types';
import { Download, Check, X } from 'lucide-react';
import { TransferQueuePanel } from './TransferQueuePanel';

const DOWNLOAD_ACTIVE = ['pending', 'downloading'] as const;

const PANEL_CLASS =
    'fixed bottom-[22rem] right-4 w-80 bg-telegram-surface border border-telegram-border rounded-xl shadow-2xl overflow-hidden z-[100]';

interface DownloadQueueProps {
    items: DownloadItem[];
    onClearFinished: () => void;
    onCancelAll: () => void;
    onCancelItem: (id: string) => void;
    onRetryItem: (id: string) => void;
}

function downloadStatusIndicator(item: DownloadItem) {
    return (
        <div className="flex-shrink-0">
            {item.status === 'pending' && (
                <div className="w-4 h-4 rounded-full bg-yellow-500/20 flex items-center justify-center">
                    <div className="w-2 h-2 bg-yellow-500 rounded-full" />
                </div>
            )}
            {item.status === 'downloading' && (
                <div className="w-4 h-4 rounded-full border-2 border-telegram-secondary border-t-transparent animate-spin" />
            )}
            {item.status === 'success' && (
                <div className="w-4 h-4 rounded-full bg-green-500/20 flex items-center justify-center">
                    <Check className="w-3 h-3 text-green-500" />
                </div>
            )}
            {item.status === 'error' && (
                <div className="w-4 h-4 rounded-full bg-red-500/20 flex items-center justify-center">
                    <X className="w-3 h-3 text-red-500" />
                </div>
            )}
            {item.status === 'cancelled' && (
                <div className="w-4 h-4 rounded-full bg-gray-500/20 flex items-center justify-center">
                    <X className="w-3 h-3 text-gray-400" />
                </div>
            )}
        </div>
    );
}

export function DownloadQueue({
    items,
    onClearFinished,
    onCancelAll,
    onCancelItem,
    onRetryItem,
}: DownloadQueueProps) {
    const activeCount = items.filter(
        (i) => i.status === 'pending' || i.status === 'downloading',
    ).length;

    return (
        <TransferQueuePanel
            items={items}
            panelClassName={PANEL_CLASS}
            ariaLabel="Download queue"
            title="Downloads"
            titlePrefix={<Download className="w-4 h-4 text-telegram-secondary" />}
            titleSuffix={
                activeCount > 0 ? (
                    <span className="text-xs px-1.5 py-0.5 bg-telegram-secondary/20 text-telegram-secondary rounded-full">
                        {activeCount} active
                    </span>
                ) : undefined
            }
            activeStatuses={DOWNLOAD_ACTIVE}
            inProgressStatus="downloading"
            progressBarClassName="bg-telegram-secondary"
            getItemLabel={(item) => item.filename}
            getItemTooltip={(item) => item.filename}
            renderStatusIndicator={downloadStatusIndicator}
            onClearFinished={onClearFinished}
            onCancelAll={onCancelAll}
            onCancelItem={onCancelItem}
            onRetryItem={onRetryItem}
        />
    );
}
