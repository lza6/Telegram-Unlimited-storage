import { useEffect, useState } from 'react';
import { X, ChevronLeft, ChevronRight } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { TelegramFile } from '../../types';
import { isVideoFile, isAudioFile } from '../../utils';
import { deriveStreamUiPhase, streamStatusMessage } from '../../lib/transferUiPure';

interface StreamInfo {
    token: string;
    base_url: string;
}

interface MediaPlayerProps {
    file: TelegramFile;
    onClose: () => void;
    onNext?: () => void;
    onPrev?: () => void;
    currentIndex?: number;
    totalItems?: number;
    activeFolderId: number | null;
}

export function MediaPlayer({ file, onClose, onNext, onPrev, currentIndex, totalItems, activeFolderId }: MediaPlayerProps) {
    const peerFolderId = file.folder_id ?? activeFolderId ?? null;
    const [streamInfo, setStreamInfo] = useState<StreamInfo | null>(null);
    const [streamError, setStreamError] = useState<string | null>(null);
    const [isBuffering, setIsBuffering] = useState(false);

    useEffect(() => {
        setStreamInfo(null);
        setStreamError(null);
        setIsBuffering(false);
        invoke<StreamInfo>('cmd_get_stream_info')
            .then(setStreamInfo)
            .catch((err) => {
                console.error('Failed to get stream info:', err);
                setStreamError('Failed to initialize media stream');
            });
    }, [file.id]);

    const folderIdParam = peerFolderId !== null ? peerFolderId.toString() : 'home';
    const streamUrl = streamInfo
        ? `${streamInfo.base_url}/stream/${folderIdParam}/${file.id}?token=${streamInfo.token}`
        : null;

    const isVideo = isVideoFile(file.name);
    const isAudio = isAudioFile(file.name);
    const streamPhase = deriveStreamUiPhase({ streamError, streamUrl, isBuffering });
    const overlayMessage = streamError ?? streamStatusMessage(streamPhase);

    const streamMediaHandlers = {
        onWaiting: () => setIsBuffering(true),
        onPlaying: () => setIsBuffering(false),
        onCanPlay: () => setIsBuffering(false),
        onError: () => setStreamError('Stream playback failed'),
    };

    useEffect(() => {
        const handleKeyDown = (e: KeyboardEvent) => {
            const target = e.target as HTMLElement;
            if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
                return;
            }

            const key = e.key.toLowerCase();

            if (e.key === 'ArrowRight' || key === 'l') {
                e.preventDefault();
                onNext?.();
                return;
            }

            if (e.key === 'ArrowLeft' || key === 'j') {
                e.preventDefault();
                onPrev?.();
                return;
            }

            if (e.key === 'Escape') {
                e.preventDefault();
                onClose();
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [onClose, onNext, onPrev]);

    return (
        <div className="fixed inset-0 z-[200] bg-black/90 flex items-center justify-center p-4 backdrop-blur-md animate-in fade-in duration-200" onClick={onClose}>
            <div className="relative w-full max-w-6xl flex flex-col items-center" onClick={e => e.stopPropagation()}>
                <button
                    onClick={onPrev}
                    className="absolute left-2 top-1/2 -translate-y-1/2 p-2 text-white/50 hover:text-white bg-white/10 hover:bg-white/20 rounded-full transition-all z-10"
                    title="Previous (ArrowLeft / J)"
                >
                    <ChevronLeft className="w-6 h-6" />
                </button>

                <button
                    onClick={onNext}
                    className="absolute right-2 top-1/2 -translate-y-1/2 p-2 text-white/50 hover:text-white bg-white/10 hover:bg-white/20 rounded-full transition-all z-10"
                    title="Next (ArrowRight / L)"
                >
                    <ChevronRight className="w-6 h-6" />
                </button>

                <button
                    onClick={onClose}
                    className="absolute -top-12 right-0 p-2 text-white/50 hover:text-white bg-white/10 hover:bg-white/20 rounded-full transition-all"
                >
                    <X className="w-6 h-6" />
                </button>

                <div className="relative w-full aspect-video bg-black rounded-xl overflow-hidden shadow-2xl ring-1 ring-white/10 flex items-center justify-center">
                    {streamPhase === 'error' ? (
                        <div className="flex flex-col items-center gap-3 text-white px-6 text-center">
                            <p className="text-red-400">{streamError}</p>
                            <button
                                type="button"
                                onClick={onClose}
                                className="px-4 py-2 rounded-lg bg-white/10 hover:bg-white/20 text-sm"
                            >
                                Close
                            </button>
                        </div>
                    ) : streamPhase === 'loading' ? (
                        <div className="flex flex-col items-center gap-4 text-white">
                            <div className="w-10 h-10 border-4 border-telegram-primary border-t-transparent rounded-full animate-spin"></div>
                            <p>{overlayMessage}</p>
                        </div>
                    ) : isVideo ? (
                        <>
                            <video
                                src={streamUrl!}
                                controls
                                autoPlay
                                className="w-full h-full object-contain"
                                {...streamMediaHandlers}
                            />
                            {streamPhase === 'buffering' && overlayMessage && (
                                <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-black/50 text-white pointer-events-none">
                                    <div className="w-10 h-10 border-4 border-telegram-primary border-t-transparent rounded-full animate-spin"></div>
                                    <p>{overlayMessage}</p>
                                </div>
                            )}
                        </>
                    ) : isAudio ? (
                        <div className="w-full h-full flex flex-col items-center justify-center bg-gradient-to-br from-telegram-primary/20 to-black">
                            <div className="w-32 h-32 rounded-full bg-telegram-surface flex items-center justify-center mb-8 shadow-xl animate-pulse-slow">
                                <svg xmlns="http://www.w3.org/2000/svg" className="w-12 h-12 text-telegram-primary" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M9 18V5l12-2v13" /><circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" /></svg>
                            </div>
                            {streamPhase === 'buffering' && overlayMessage && (
                                <p className="text-white/80 text-sm mb-4">{overlayMessage}</p>
                            )}
                            <audio src={streamUrl!} controls autoPlay className="w-full max-w-md" {...streamMediaHandlers} />
                        </div>
                    ) : (
                        <div className="text-white">Unsupported media type</div>
                    )}
                </div>

                <div className="mt-4 text-center">
                    <h3 className="text-lg font-medium text-white">{file.name}</h3>
                    <p className="text-sm text-white/50">
                        Streaming from Telegram Drive
                        {typeof currentIndex === 'number' && typeof totalItems === 'number' && totalItems > 0 && (
                            <span className="ml-2">• {currentIndex + 1}/{totalItems}</span>
                        )}
                    </p>
                </div>
            </div>
        </div>
    );
}
