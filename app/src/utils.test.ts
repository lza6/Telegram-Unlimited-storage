import { describe, it, expect } from 'vitest';
import {
    formatBytes, isMediaFile, isVideoFile, isAudioFile, isImageFile, isPdfFile,
    resolveFileFolderId, groupIdsBySourceFolder, fileBelongsToFolder, idsIncludeOpenFile,
    pruneSelectedIdsAfterDelete, filterFilesExcludingIds, remapMovedFilesInList, remapOpenFileAfterMove,
    planMoveGroups, mergeMovePayloads,
} from './utils';

describe('formatBytes', () => {
    it('returns 0 Bytes for zero', () => {
        expect(formatBytes(0)).toBe('0 Bytes');
    });

    it('formats bytes correctly', () => {
        expect(formatBytes(512)).toBe('512 Bytes');
    });

    it('formats kilobytes correctly', () => {
        expect(formatBytes(1024)).toBe('1 KB');
        expect(formatBytes(1536)).toBe('1.5 KB');
    });

    it('formats megabytes correctly', () => {
        expect(formatBytes(1048576)).toBe('1 MB');
        expect(formatBytes(1572864)).toBe('1.5 MB');
    });

    it('formats gigabytes correctly', () => {
        expect(formatBytes(1073741824)).toBe('1 GB');
    });

    it('respects decimal parameter', () => {
        expect(formatBytes(1536, 0)).toBe('2 KB');
        expect(formatBytes(1536, 1)).toBe('1.5 KB');
    });
});

describe('file type detection', () => {
    describe('isVideoFile', () => {
        it('detects video extensions', () => {
            expect(isVideoFile('movie.mp4')).toBe(true);
            expect(isVideoFile('clip.webm')).toBe(true);
            expect(isVideoFile('video.MOV')).toBe(true); // case insensitive
        });

        it('rejects non-video files', () => {
            expect(isVideoFile('song.mp3')).toBe(false);
            expect(isVideoFile('doc.pdf')).toBe(false);
        });
    });

    describe('isAudioFile', () => {
        it('detects audio extensions', () => {
            expect(isAudioFile('song.mp3')).toBe(true);
            expect(isAudioFile('track.flac')).toBe(true);
            expect(isAudioFile('audio.WAV')).toBe(true);
        });

        it('rejects non-audio files', () => {
            expect(isAudioFile('movie.mp4')).toBe(false);
        });
    });

    describe('isMediaFile', () => {
        it('detects both video and audio', () => {
            expect(isMediaFile('movie.mp4')).toBe(true);
            expect(isMediaFile('song.mp3')).toBe(true);
        });

        it('rejects non-media files', () => {
            expect(isMediaFile('doc.pdf')).toBe(false);
            expect(isMediaFile('image.jpg')).toBe(false);
        });
    });

    describe('isImageFile', () => {
        it('detects image extensions', () => {
            expect(isImageFile('photo.jpg')).toBe(true);
            expect(isImageFile('pic.png')).toBe(true);
            expect(isImageFile('art.webp')).toBe(true);
            expect(isImageFile('shot.HEIC')).toBe(true);
        });

        it('rejects non-image files', () => {
            expect(isImageFile('doc.pdf')).toBe(false);
        });
    });

    describe('isPdfFile', () => {
        it('detects pdf extension', () => {
            expect(isPdfFile('document.pdf')).toBe(true);
            expect(isPdfFile('report.PDF')).toBe(true);
        });

        it('rejects non-pdf files', () => {
            expect(isPdfFile('doc.txt')).toBe(false);
        });
    });
});

describe('resolveFileFolderId', () => {
    it('prefers file.folder_id over active folder', () => {
        expect(resolveFileFolderId({ folder_id: 42 }, 100)).toBe(42);
    });

    it('falls back to activeFolderId when folder_id missing', () => {
        expect(resolveFileFolderId({}, 100)).toBe(100);
        expect(resolveFileFolderId({ folder_id: null }, null)).toBe(null);
    });
});

describe('groupIdsBySourceFolder', () => {
    it('groups ids by each file folder_id', () => {
        const files = [
            { id: 1, folder_id: 10 },
            { id: 2, folder_id: 20 },
            { id: 3, folder_id: 10 },
        ];
        const groups = groupIdsBySourceFolder([1, 2, 3], files, null);
        expect(groups.size).toBe(2);
        expect(groups.get('10')?.ids).toEqual([1, 3]);
        expect(groups.get('20')?.ids).toEqual([2]);
    });
});

describe('fileBelongsToFolder', () => {
    it('matches indexed folder_id', () => {
        expect(fileBelongsToFolder({ folder_id: 5 }, 5, 99)).toBe(true);
        expect(fileBelongsToFolder({ folder_id: 5 }, 6, 99)).toBe(false);
    });

    it('falls back to active folder when folder_id missing', () => {
        expect(fileBelongsToFolder({}, 10, 10)).toBe(true);
    });
});

describe('idsIncludeOpenFile', () => {
    it('detects open preview file in id list', () => {
        expect(idsIncludeOpenFile([1, 2, 3], { id: 2 })).toBe(true);
        expect(idsIncludeOpenFile([1, 3], { id: 2 })).toBe(false);
        expect(idsIncludeOpenFile([1], null)).toBe(false);
    });
});

describe('pruneSelectedIdsAfterDelete', () => {
    it('keeps failed delete ids selected for retry', () => {
        expect(pruneSelectedIdsAfterDelete([1, 2, 3], [2])).toEqual([1, 3]);
        expect(pruneSelectedIdsAfterDelete([1, 2], [])).toEqual([1, 2]);
    });
});

describe('filterFilesExcludingIds', () => {
    it('drops moved or deleted ids from search/preview lists', () => {
        const files = [{ id: 1, name: 'a' }, { id: 2, name: 'b' }, { id: 3, name: 'c' }];
        expect(filterFilesExcludingIds(files, [2])).toEqual([
            { id: 1, name: 'a' },
            { id: 3, name: 'c' },
        ]);
    });
});

describe('remapMovedFilesInList', () => {
    it('updates id and folder_id after Telegram forward move', () => {
        const files = [{ id: 10, name: 'a', folder_id: 1 }, { id: 99, name: 'b', folder_id: null }];
        const remapped = remapMovedFilesInList(files, {
            oldIds: [10],
            newIds: [50],
            targetFolderId: 7,
        });
        expect(remapped).toEqual([
            { id: 50, name: 'a', folder_id: 7 },
            { id: 99, name: 'b', folder_id: null },
        ]);
    });
});

describe('remapOpenFileAfterMove', () => {
    it('returns null when mapping missing', () => {
        expect(remapOpenFileAfterMove({ id: 1, name: 'x' }, { oldIds: [1], newIds: [], targetFolderId: 2 })).toBeNull();
    });
});

describe('planMoveGroups', () => {
    const files = [{ id: 1, folder_id: 10 }, { id: 2, folder_id: null }];

    it('skips groups already in target folder', () => {
        const groups = planMoveGroups([1, 2], files, null, 10);
        expect(groups).toEqual([{ sourceFolderId: null, ids: [2] }]);
    });
});

describe('mergeMovePayloads', () => {
    it('concatenates multi-group move results', () => {
        const merged = mergeMovePayloads([
            { oldIds: [1], newIds: [10], targetFolderId: 5 },
            { oldIds: [2], newIds: [20], targetFolderId: 5 },
        ]);
        expect(merged).toEqual({
            oldIds: [1, 2],
            newIds: [10, 20],
            targetFolderId: 5,
        });
    });
});
