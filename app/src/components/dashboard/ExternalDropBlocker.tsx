import { useState, useEffect } from 'react';
import { Upload } from 'lucide-react';
import type { UnlistenFn } from '@tauri-apps/api/event';

/**
 * External drop handler — Tauri OS file paths enqueue upload; browser dev falls back to Upload button.
 */
export function ExternalDropBlocker({
    onUploadPaths,
    onUploadClick,
    uploadEnabled = true,
    onUploadBlocked,
}: {
    onUploadPaths: (paths: string[]) => void;
    onUploadClick: () => void;
    uploadEnabled?: boolean;
    onUploadBlocked?: () => void;
}) {
    const [showMessage, setShowMessage] = useState(false);
    const [tauriDropSupported, setTauriDropSupported] = useState(false);

    useEffect(() => {
        let unlisten: UnlistenFn | undefined;
        let cancelled = false;

        (async () => {
            try {
                const { getCurrentWebview } = await import('@tauri-apps/api/webview');
                const webview = getCurrentWebview();
                unlisten = await webview.onDragDropEvent((event) => {
                    if (event.payload.type === 'enter') {
                        setShowMessage(true);
                    } else if (event.payload.type === 'leave') {
                        setShowMessage(false);
                    } else if (event.payload.type === 'drop') {
                        setShowMessage(false);
                        const paths = event.payload.paths ?? [];
                        if (paths.length > 0) {
                            if (uploadEnabled) {
                                onUploadPaths(paths);
                            } else {
                                onUploadBlocked?.();
                            }
                        }
                    }
                });
                if (!cancelled) setTauriDropSupported(true);
            } catch {
                // Vite dev in browser — DOM fallback below
            }
        })();

        return () => {
            cancelled = true;
            unlisten?.();
        };
    }, [onUploadPaths, uploadEnabled, onUploadBlocked]);

    useEffect(() => {
        if (tauriDropSupported) return;

        let hideTimeout: ReturnType<typeof setTimeout>;

        const handleDragOver = (e: DragEvent) => {
            if (e.dataTransfer?.types.includes('Files')) {
                e.preventDefault();
                e.stopPropagation();
                setShowMessage(true);
                clearTimeout(hideTimeout);
            }
        };

        const handleDragLeave = (e: DragEvent) => {
            if (
                e.clientX <= 0 || e.clientY <= 0 ||
                e.clientX >= window.innerWidth || e.clientY >= window.innerHeight
            ) {
                hideTimeout = setTimeout(() => setShowMessage(false), 100);
            }
        };

        const handleDrop = (e: DragEvent) => {
            if (e.dataTransfer?.types.includes('Files')) {
                e.preventDefault();
                e.stopPropagation();
                setTimeout(() => setShowMessage(false), 2000);
                if (!uploadEnabled) {
                    onUploadBlocked?.();
                }
            }
        };

        document.addEventListener('dragover', handleDragOver, true);
        document.addEventListener('dragleave', handleDragLeave, true);
        document.addEventListener('drop', handleDrop, true);

        return () => {
            document.removeEventListener('dragover', handleDragOver, true);
            document.removeEventListener('dragleave', handleDragLeave, true);
            document.removeEventListener('drop', handleDrop, true);
            clearTimeout(hideTimeout);
        };
    }, [tauriDropSupported, uploadEnabled, onUploadBlocked]);

    if (!showMessage) return null;

    const isTauri = tauriDropSupported;

    return (
        <div className="fixed inset-0 z-[100] bg-black/60 backdrop-blur-sm flex items-center justify-center pointer-events-none">
            <div className="glass bg-telegram-surface border border-telegram-border rounded-2xl p-8 max-w-md mx-4 shadow-2xl pointer-events-auto">
                <div className="flex flex-col items-center text-center gap-4">
                    <div className="w-16 h-16 rounded-full bg-telegram-primary/20 flex items-center justify-center">
                        <Upload className="w-8 h-8 text-telegram-primary" />
                    </div>
                    <div>
                        <h3 className="text-lg font-semibold text-telegram-text mb-2">
                            {isTauri ? 'Release to Upload' : 'Use the Upload Button'}
                        </h3>
                        <p className="text-telegram-subtext text-sm">
                            {isTauri ? (
                                <>松开鼠标将文件加入上传队列。</>
                            ) : (
                                <>
                                    To upload in browser dev, use the <strong>Upload</strong> button in the toolbar.
                                </>
                            )}
                        </p>
                    </div>
                    {!isTauri && (
                        <button
                            onClick={() => {
                                setShowMessage(false);
                                if (uploadEnabled) onUploadClick();
                            }}
                            disabled={!uploadEnabled}
                            className="mt-2 px-6 py-2 bg-telegram-primary text-white rounded-lg font-medium hover:bg-telegram-primary/90 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                        >
                            Open Upload Dialog
                        </button>
                    )}
                </div>
            </div>
        </div>
    );
}
