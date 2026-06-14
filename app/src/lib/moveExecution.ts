import { invoke } from '@tauri-apps/api/core';
import { MoveFilesPayload, MoveFilesResult } from '../types';
import { mergeMovePayloads, moveResultToPayload } from '../utils';

export type MoveGroup = { sourceFolderId: number | null; ids: number[] };

export type ExecuteMoveGroupsResult = {
    moved: number;
    movedOldIds: number[];
    mergedPayload: MoveFilesPayload | null;
    failures: string[];
};

export async function executeMoveGroups(
    groups: MoveGroup[],
    targetFolderId: number | null,
): Promise<ExecuteMoveGroupsResult> {
    let moved = 0;
    const movePayloads: MoveFilesPayload[] = [];
    const movedOldIds: number[] = [];
    const failures: string[] = [];

    for (const { sourceFolderId, ids } of groups) {
        try {
            const result = await invoke<MoveFilesResult>('cmd_move_files', {
                messageIds: ids,
                sourceFolderId,
                targetFolderId,
            });
            moved += result.moved;
            if (result.moved > 0) {
                movePayloads.push(moveResultToPayload(result));
                movedOldIds.push(...result.oldMessageIds);
            }
        } catch (err) {
            failures.push(String(err));
        }
    }

    return {
        moved,
        movedOldIds,
        mergedPayload: mergeMovePayloads(movePayloads),
        failures,
    };
}
