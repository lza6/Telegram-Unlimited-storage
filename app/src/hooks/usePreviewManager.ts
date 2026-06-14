import { useState, useCallback } from 'react';
import { TelegramFile } from '../types';
import { isMediaFile, isPdfFile, filterFilesExcludingIds, remapMovedFilesInList, remapOpenFileAfterMove } from '../utils';

export interface UsePreviewManagerReturn {
  previewFile: TelegramFile | null;
  playingFile: TelegramFile | null;
  pdfFile: TelegramFile | null;
  shareFile: TelegramFile | null;
  previewContextFiles: TelegramFile[];
  previewContextIndex: number;
  handlePreview: (file: TelegramFile, orderedFiles?: TelegramFile[]) => void;
  navigatePreview: (step: 1 | -1) => void;
  handleNextPreview: () => void;
  handlePrevPreview: () => void;
  previewNeighborFiles: () => { nextFile: TelegramFile | null; prevFile: TelegramFile | null };
  closePreviewState: () => void;
  closePreviewIfRemoved: (removedIds: number[]) => void;
  setShareFile: (file: TelegramFile | null) => void;
  handleFilesRemoved: (removedIds: number[]) => void;
  handleFilesMoved: (payload: { oldIds: number[]; newIds: number[]; targetFolderId: number | null }) => void;
  setPreviewContextFiles: React.Dispatch<React.SetStateAction<TelegramFile[]>>;
  setPreviewContextIndex: React.Dispatch<React.SetStateAction<number>>;
  setPreviewFile: React.Dispatch<React.SetStateAction<TelegramFile | null>>;
  setPlayingFile: React.Dispatch<React.SetStateAction<TelegramFile | null>>;
  setPdfFile: React.Dispatch<React.SetStateAction<TelegramFile | null>>;
}

export function usePreviewManager(
  displayedFiles: TelegramFile[],
  previewReady: boolean,
  _previewBlockedMessage: string | undefined,
  _transferBlockedMessage: string | undefined,
): UsePreviewManagerReturn {
  const [previewFile, setPreviewFile] = useState<TelegramFile | null>(null);
  const [playingFile, setPlayingFile] = useState<TelegramFile | null>(null);
  const [pdfFile, setPdfFile] = useState<TelegramFile | null>(null);
  const [shareFile, setShareFile] = useState<TelegramFile | null>(null);
  const [previewContextFiles, setPreviewContextFiles] = useState<TelegramFile[]>([]);
  const [previewContextIndex, setPreviewContextIndex] = useState(-1);

  const closePreviewState = useCallback(() => {
    setPreviewFile(null);
    setPlayingFile(null);
    setPdfFile(null);
  }, []);

  const closePreviewIfRemoved = useCallback((removedIds: number[]) => {
    const openId = previewFile?.id ?? playingFile?.id ?? pdfFile?.id ?? shareFile?.id;
    if (openId && removedIds.includes(openId)) {
      closePreviewState();
      setShareFile(null);
    }
  }, [previewFile, playingFile, pdfFile, shareFile, closePreviewState]);

  const handleFilesRemoved = useCallback((removedIds: number[]) => {
    closePreviewIfRemoved(removedIds);
    setPreviewContextFiles((prev) => filterFilesExcludingIds(prev, removedIds));
  }, [closePreviewIfRemoved]);

  const handleFilesMoved = useCallback((payload: { oldIds: number[]; newIds: number[]; targetFolderId: number | null }) => {
    setPreviewContextFiles((prev) => remapMovedFilesInList(prev, payload));
    setPreviewFile((prev) => remapOpenFileAfterMove(prev, payload));
    setPlayingFile((prev) => remapOpenFileAfterMove(prev, payload));
    setPdfFile((prev) => remapOpenFileAfterMove(prev, payload));
    setShareFile((prev) => remapOpenFileAfterMove(prev, payload));
  }, []);

  const handlePreview = useCallback((file: TelegramFile, orderedFiles?: TelegramFile[]) => {
    if (file.type !== 'folder' && !previewReady) {
      // toast handled by caller
      return;
    }
    const contextFiles = (orderedFiles || displayedFiles).filter((f) => f.type !== 'folder');
    const contextIndex = contextFiles.findIndex((f) => f.id === file.id);

    setPreviewContextFiles(contextFiles);
    setPreviewContextIndex(contextIndex);

    const isMedia = isMediaFile(file.name);
    const isPdf = isPdfFile(file.name);

    if (isMedia) {
      setPlayingFile(file);
      setPreviewFile(null);
      setPdfFile(null);
    } else if (isPdf) {
      setPdfFile(file);
      setPreviewFile(null);
      setPlayingFile(null);
    } else {
      setPreviewFile(file);
      setPlayingFile(null);
      setPdfFile(null);
    }
  }, [displayedFiles, previewReady]);

  const navigatePreview = useCallback((step: 1 | -1) => {
    if (!previewReady) return;
    if (previewContextFiles.length === 0) return;

    const currentFileId = previewFile?.id ?? playingFile?.id ?? pdfFile?.id;
    if (!currentFileId) return;

    const currentIndex = previewContextFiles.findIndex((f) => f.id === currentFileId);
    if (currentIndex === -1) return;

    const nextIndex = (currentIndex + step + previewContextFiles.length) % previewContextFiles.length;
    const nextFile = previewContextFiles[nextIndex];
    if (!nextFile) return;

    setPreviewContextIndex(nextIndex);

    const isMedia = isMediaFile(nextFile.name);
    const isPdf = isPdfFile(nextFile.name);

    if (isMedia) {
      setPlayingFile(nextFile);
      setPreviewFile(null);
      setPdfFile(null);
    } else if (isPdf) {
      setPdfFile(nextFile);
      setPreviewFile(null);
      setPlayingFile(null);
    } else {
      setPreviewFile(nextFile);
      setPlayingFile(null);
      setPdfFile(null);
    }
  }, [previewContextFiles, previewFile, playingFile, pdfFile, previewReady]);

  const handleNextPreview = useCallback(() => navigatePreview(1), [navigatePreview]);
  const handlePrevPreview = useCallback(() => navigatePreview(-1), [navigatePreview]);

  const previewNeighborFiles = useCallback(() => {
    if (previewContextFiles.length === 0) {
      return { nextFile: null as TelegramFile | null, prevFile: null as TelegramFile | null };
    }

    const currentFileId = previewFile?.id ?? playingFile?.id ?? pdfFile?.id;
    if (!currentFileId) {
      return { nextFile: null as TelegramFile | null, prevFile: null as TelegramFile | null };
    }

    const currentIdx = previewContextFiles.findIndex((f) => f.id === currentFileId);
    if (currentIdx === -1) {
      return { nextFile: null as TelegramFile | null, prevFile: null as TelegramFile | null };
    }

    const nextIdx = (currentIdx + 1) % previewContextFiles.length;
    const prevIdx = (currentIdx - 1 + previewContextFiles.length) % previewContextFiles.length;

    return {
      nextFile: previewContextFiles[nextIdx] || null,
      prevFile: previewContextFiles[prevIdx] || null,
    };
  }, [previewContextFiles, previewFile, playingFile, pdfFile]);

  return {
    previewFile,
    playingFile,
    pdfFile,
    shareFile,
    previewContextFiles,
    previewContextIndex,
    handlePreview,
    navigatePreview,
    handleNextPreview,
    handlePrevPreview,
    previewNeighborFiles,
    closePreviewState,
    closePreviewIfRemoved,
    setShareFile,
    handleFilesRemoved,
    handleFilesMoved,
    setPreviewContextFiles,
    setPreviewContextIndex,
    setPreviewFile,
    setPlayingFile,
    setPdfFile,
  };
}
