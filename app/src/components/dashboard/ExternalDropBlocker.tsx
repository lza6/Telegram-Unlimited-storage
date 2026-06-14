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
    const [dropCount, setDropCount] = useState(0);
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
                        const paths = event.payload.paths ?? [];
                        setDropCount(paths.length || 0);
                    } else if (event.payload.type === 'leave') {
                        setShowMessage(false);
                        setDropCount(0);
                    } else if (event.payload.type === 'drop') {
                        setShowMessage(false);
                        setDropCount(0);
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
        <div className="fixed inset-0 z-[100] bg-black/60 backdrop-blur-sm flex items-center justify-center pointer-events-none animate-in fade-in duration-200">
            <div className="glass bg-telegram-surface border-2 border-dashed border-telegram-primary rounded-2xl p-8 max-w-md mx-4 shadow-2xl pointer-events-auto animate-in zoom-in-95 duration-200">
                <div className="flex flex-col items-center text-center gap-4">
                    <div className="w-20 h-20 rounded-full bg-telegram-primary/20 flex items-center justify-center animate-pulse">
                        <Upload className="w-10 h-10 text-telegram-primary" />
                    </div>
                    {dropCount > 0 && (
                        <div className="absolute -top-2 -right-2 w-8 h-8 bg-telegram-primary rounded-full flex items-center justify-center text-white font-bold text-sm shadow-lg">
                            {dropCount > 99 ? '99+' : dropCount}
                        </div>
                    )}
                    <div>
                        <h3 className="text-lg font-semibold text-telegram-text mb-2">
                            {isTauri ? '松开上传' : '使用上传按钮'}
                        </h3>
                        <p className="text-telegram-subtext text-sm">
                            {isTauri ? (
                                dropCount > 0 ? (
                                    <>拖入 <strong className="text-telegram-primary">{dropCount}</strong> 个文件，松开加入上传队列。</>
                                ) : (
                                    <>松开鼠标将文件加入上传队列。</>
                                )
                            ) : (
                                <>
                                    浏览器开发模式请使用工具栏的 <strong>上传</strong> 按钮。
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
                            打开上传对话框
                        </button>
                    )}
                </div>
            </div>
        </div>
    );
}
