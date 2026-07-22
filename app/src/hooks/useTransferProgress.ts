import { useEffect, useRef } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

/**
 * Common progress payload structure for upload/download events.
 */
export interface ProgressPayload {
    id: string;
    percent: number;
    uploaded_bytes: number;
    total_bytes: number;
    speed_bytes_per_sec: number;
}

/**
 * Shared hook for listening to transfer progress events.
 * Used by both useFileUpload and useFileDownload.
 *
 * @param eventName - The event name to listen for ('upload-progress' or 'download-progress')
 * @param onProgress - Callback function receiving the progress payload
 */
export function useTransferProgress(
    eventName: 'upload-progress' | 'download-progress',
    onProgress: (payload: ProgressPayload) => void,
): void {
    const onProgressRef = useRef(onProgress);
    onProgressRef.current = onProgress;

    useEffect(() => {
        let unlisten: UnlistenFn | undefined;

        listen<ProgressPayload>(eventName, (event) => {
            onProgressRef.current(event.payload);
        }).then(fn => {
            unlisten = fn;
        });

        return () => {
            unlisten?.();
        };
    }, [eventName]);
}

/**
 * Creates a progress updater function for queue items.
 * Returns a function that updates a specific item's progress in the queue.
 */
export function createProgressUpdater<T extends { id: string }>(
    setQueue: React.Dispatch<React.SetStateAction<T[]>>,
): (id: string, updates: Partial<T>) => void {
    return (id: string, updates: Partial<T>) => {
        setQueue(q => q.map(i =>
            i.id === id ? { ...i, ...updates } : i
        ));
    };
}
