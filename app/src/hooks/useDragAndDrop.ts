import { useState, useRef, useCallback } from 'react';
import { toast } from 'sonner';
import { planMoveGroups, pruneSelectedIdsAfterDelete } from '../utils';
import { executeMoveGroups } from '../lib/moveExecution';

export interface UseDragAndDropReturn {
  internalDragFileId: number | null;
  internalDragRef: React.MutableRefObject<number | null>;
  setInternalDragFileId: React.Dispatch<React.SetStateAction<number | null>>;
  handleDropOnFolder: (
    e: React.DragEvent,
    targetFolderId: number | null,
    opts: {
      activeFolderId: number | null;
      selectedIds: number[];
      displayedFiles: { id: number; folder_id?: number | null }[];
      sessionOnline: boolean;
      transferBlockedMessage: string | undefined;
      bulkMoveAllowed: boolean;
      bulkMoveBlockedMessage: string;
      onFilesMoved: (payload: { oldIds: number[]; newIds: number[]; targetFolderId: number | null }) => void;
      onSessionError: (msg: string) => void;
      invalidateFiles: () => void;
      setSelectedIds: React.Dispatch<React.SetStateAction<number[]>>;
    },
  ) => Promise<void>;
  handleRootDragOver: (e: React.DragEvent) => void;
  handleRootDragEnter: (e: React.DragEvent) => void;
}

export function useDragAndDrop(): UseDragAndDropReturn {
  const [internalDragFileId, setInternalDragFileId] = useState<number | null>(null);
  const internalDragRef = useRef<number | null>(null);
  // Sync ref on every change so handlers always read latest
  internalDragRef.current = internalDragFileId;

  const handleDropOnFolder = useCallback(async (
    e: React.DragEvent,
    targetFolderId: number | null,
    opts: {
      activeFolderId: number | null;
      selectedIds: number[];
      displayedFiles: { id: number; folder_id?: number | null }[];
      sessionOnline: boolean;
      transferBlockedMessage: string | undefined;
      bulkMoveAllowed: boolean;
      bulkMoveBlockedMessage: string;
      onFilesMoved: (payload: { oldIds: number[]; newIds: number[]; targetFolderId: number | null }) => void;
      onSessionError: (msg: string) => void;
      invalidateFiles: () => void;
      setSelectedIds: React.Dispatch<React.SetStateAction<number[]>>;
    },
  ) => {
    e.preventDefault();
    e.stopPropagation();

    if (!opts.sessionOnline) {
      toast.error(opts.transferBlockedMessage);
      return;
    }
    if (!opts.bulkMoveAllowed) {
      toast.error(opts.bulkMoveBlockedMessage);
      return;
    }

    const dataTransferFileId = e.dataTransfer.getData('application/x-telegram-file-id');

    if (opts.activeFolderId === targetFolderId) return;

    const fileId = internalDragRef.current || (dataTransferFileId ? parseInt(dataTransferFileId) : null);

    if (fileId) {
      const idsToMove = opts.selectedIds.includes(fileId) ? opts.selectedIds : [fileId];
      const groups = planMoveGroups(idsToMove, opts.displayedFiles, opts.activeFolderId, targetFolderId);
      const { moved, movedOldIds, mergedPayload, failures } = await executeMoveGroups(
        groups,
        targetFolderId,
      );

      if (movedOldIds.length > 0) {
        opts.invalidateFiles();
        if (mergedPayload) opts.onFilesMoved(mergedPayload);
        opts.setSelectedIds((prev) => pruneSelectedIdsAfterDelete(prev, movedOldIds));
      }

      if (moved > 0) toast.success(`已移动 ${moved} 个文件`);
      if (failures.length > 0) {
        const detail = failures.length === groups.length
          ? failures[0]
          : `部分移动失败（${failures.length}/${groups.length}）：${failures[0]}`;
        toast.error(`移动文件失败: ${detail}`);
        const sessionErr = failures.find((f) => f.includes('session') || f.includes('SESSION') || f.includes('AUTH'));
        if (sessionErr) opts.onSessionError(sessionErr);
      } else if (moved === 0) {
        toast.info('文件已在此文件夹中');
      }

      setInternalDragFileId(null);
    }
  }, []);

  const handleRootDragOver = useCallback((e: React.DragEvent) => {
    if (internalDragRef.current) {
      e.preventDefault();
      e.stopPropagation();
      e.dataTransfer.dropEffect = 'move';
    }
  }, []);

  const handleRootDragEnter = useCallback((e: React.DragEvent) => {
    if (internalDragRef.current) {
      e.preventDefault();
      e.stopPropagation();
      e.dataTransfer.dropEffect = 'move';
    }
  }, []);

  return {
    internalDragFileId,
    internalDragRef,
    setInternalDragFileId,
    handleDropOnFolder,
    handleRootDragOver,
    handleRootDragEnter,
  };
}
