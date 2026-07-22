import { QueueItem } from '../../types';
import { TransferQueuePanel } from './TransferQueuePanel';

const UPLOAD_ACTIVE = ['pending', 'uploading'] as const;

const PANEL_CLASS =
    'fixed bottom-4 right-4 w-80 bg-telegram-surface border border-telegram-border rounded-xl shadow-2xl overflow-hidden z-[100]';

interface UploadQueueProps {
    items: QueueItem[];
    onClearFinished: () => void;
    onCancelAll: () => void;
    onCancelItem: (id: string) => void;
    onRetryItem: (id: string) => void;
}

function uploadStatusDot(item: QueueItem) {
    return (
        <div
            className={`w-2 h-2 rounded-full flex-shrink-0 ${
                item.status === 'pending'
                    ? 'bg-yellow-500'
                    : item.status === 'uploading'
                      ? 'bg-blue-500 animate-pulse'
                      : item.status === 'cancelled'
                        ? 'bg-gray-500'
                        : item.status === 'error'
                          ? 'bg-red-500'
                          : 'bg-green-500'
            }`}
        />
    );
}

export function UploadQueue({
    items,
    onClearFinished,
    onCancelAll,
    onCancelItem,
    onRetryItem,
}: UploadQueueProps) {
    return (
        <TransferQueuePanel
            items={items}
            panelClassName={PANEL_CLASS}
            ariaLabel="Upload queue"
            title="Uploads"
            activeStatuses={UPLOAD_ACTIVE}
            inProgressStatus="uploading"
            progressBarClassName="bg-blue-500"
            getItemLabel={(item) => item.path.split('/').pop() ?? item.path}
            getItemTooltip={(item) => item.path}
            renderStatusIndicator={uploadStatusDot}
            onClearFinished={onClearFinished}
            onCancelAll={onCancelAll}
            onCancelItem={onCancelItem}
            onRetryItem={onRetryItem}
        />
    );
}
