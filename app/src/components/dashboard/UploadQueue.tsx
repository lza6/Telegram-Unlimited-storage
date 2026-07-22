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

export function describeUploadState(item: QueueItem): string {
    const error = item.error || '';
    if (error.includes('UPLOAD_IN_PROGRESS')) return '同一上传正在处理中，可稍后安全重试';
    if (error.includes('UPLOAD_RECONCILIATION_REQUIRED')) return 'Telegram 已接收，正在等待数据库对账';
    if (error.includes('UPLOAD_COMPENSATION_PENDING')) return '上传未落账，正在等待补偿处理';
    if (/MANUAL_REVIEW|manual_review/i.test(error)) return '任务需要人工审查，请联系管理员';
    if (/SCHEDULER|COOLDOWN|FloodWait|retry-after/i.test(error)) return '任务正在排队或限流冷却';
    switch (item.status) {
        case 'pending': return '等待上传';
        case 'uploading': return '上传中';
        case 'success': return '上传完成';
        case 'cancelled': return '已取消，可重试';
        case 'error': return error || '上传失败，可重试';
    }
}

function uploadStatusDot(item: QueueItem) {
    const label = describeUploadState(item);
    return (
        <div
            role="img"
            aria-label={label}
            title={label}
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
    if (items.length === 0) return null;

    const active = items.filter((item) => UPLOAD_ACTIVE.includes(item.status as 'pending' | 'uploading')).length;
    const attention = items.filter((item) => item.status === 'error' || item.status === 'cancelled').length;
    const liveSummary = active > 0
        ? `${active} 个上传任务进行中`
        : attention > 0
          ? `${attention} 个上传任务需要处理，可使用重试按钮复用原任务标识`
          : '上传队列已完成';

    return (
        <>
            <div className="sr-only" role="status" aria-live="polite" aria-atomic="true">
                {liveSummary}
            </div>
            <TransferQueuePanel
                items={items}
                panelClassName={PANEL_CLASS}
                ariaLabel="Upload queue"
                title="Uploads"
                titleSuffix={<span className="sr-only">{liveSummary}</span>}
                activeStatuses={UPLOAD_ACTIVE}
                inProgressStatus="uploading"
                progressBarClassName="bg-blue-500"
                getItemLabel={(item) => item.path.split(/[/\\]/).pop() ?? item.path}
                getItemTooltip={(item) => `${item.path} — ${describeUploadState(item)}`}
                renderStatusIndicator={uploadStatusDot}
                onClearFinished={onClearFinished}
                onCancelAll={onCancelAll}
                onCancelItem={onCancelItem}
                onRetryItem={onRetryItem}
            />
        </>
    );
}