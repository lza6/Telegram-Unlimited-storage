import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { TelegramFile } from '../types';
import { isSessionLostError } from '../utils/sessionError';
import {
  isGlobalSearchActive,
  shouldRebuildIndexBeforeGlobalSearch,
  buildRebuildFolderIds,
  formatIndexRebuildBackgroundFailureMessage,
} from '../lib/searchPure';

export interface UseGlobalSearchReturn {
  searchTerm: string;
  setSearchTerm: React.Dispatch<React.SetStateAction<string>>;
  searchResults: TelegramFile[];
  isSearching: boolean;
  resetSearch: () => void;
  handleFilesMoved: (payload: { oldIds: number[]; newIds: number[]; targetFolderId: number | null }) => void;
  handleFilesRemoved: (removedIds: number[]) => void;
}

export function useGlobalSearch(
  serviceReady: boolean,
  botIndexReady: boolean,
  folders: { id: number }[],
  onSessionError: (msg: string) => void,
  _onTransportSwitched?: () => void,
): UseGlobalSearchReturn {
  const [searchTerm, setSearchTerm] = useState('');
  const [searchResults, setSearchResults] = useState<TelegramFile[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const globalSearchActiveRef = useRef(false);

  const resetSearch = useCallback(() => {
    setSearchTerm('');
    setSearchResults([]);
    globalSearchActiveRef.current = false;
  }, []);

  const handleFilesMoved = useCallback((payload: { oldIds: number[]; newIds: number[]; targetFolderId: number | null }) => {
    setSearchResults((prev) => {
      const idMap = new Map<number, number>();
      payload.oldIds.forEach((oldId, index) => {
        const newId = payload.newIds[index];
        if (newId != null) idMap.set(oldId, newId);
      });
      if (idMap.size === 0) {
        return prev.filter((f) => !payload.oldIds.includes(f.id));
      }
      return prev.map((file) => {
        const mapped = idMap.get(file.id);
        if (mapped == null) return file;
        return { ...file, id: mapped, folder_id: payload.targetFolderId };
      });
    });
  }, []);

  const handleFilesRemoved = useCallback((removedIds: number[]) => {
    setSearchResults((prev) => prev.filter((f) => !removedIds.includes(f.id)));
  }, []);

  useEffect(() => {
    if (!isGlobalSearchActive(searchTerm)) {
      globalSearchActiveRef.current = false;
      setSearchResults([]);
      return;
    }
    if (!serviceReady) {
      setSearchResults([]);
      return;
    }

    const timer = setTimeout(async () => {
      setIsSearching(true);
      try {
        if (shouldRebuildIndexBeforeGlobalSearch({
          botIndexMode: botIndexReady,
          wasActive: globalSearchActiveRef.current,
          term: searchTerm,
        })) {
          globalSearchActiveRef.current = true;
          try {
            const rebuilt = await invoke<{ folders_scanned: number; files_indexed: number }>(
              'cmd_rebuild_file_index',
              { folderIds: buildRebuildFolderIds(folders) },
            );
            if (rebuilt.files_indexed > 0) {
              toast.info(
                `索引重建完成: ${rebuilt.files_indexed} 个文件，${rebuilt.folders_scanned} 个文件夹`,
              );
            }
          } catch (rebuildErr) {
            toast.info(formatIndexRebuildBackgroundFailureMessage(rebuildErr));
          }
        } else {
          globalSearchActiveRef.current = true;
        }
        const results = await invoke<TelegramFile[]>('cmd_search_global', { query: searchTerm });
        setSearchResults(results);
      } catch (e) {
        const errMsg = String(e);
        toast.error(`搜索失败: ${errMsg}`);
        setSearchResults([]);
        if (isSessionLostError(errMsg)) {
          onSessionError(errMsg);
        }
      } finally {
        setIsSearching(false);
      }
    }, 500);

    return () => clearTimeout(timer);
  }, [searchTerm, serviceReady, botIndexReady, onSessionError, folders]);

  return {
    searchTerm,
    setSearchTerm,
    searchResults,
    isSearching,
    resetSearch,
    handleFilesMoved,
    handleFilesRemoved,
  };
}
