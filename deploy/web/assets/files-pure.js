/* Pure file helpers — keep in sync with app/src/lib/filesPure.ts */
(function (global) {
  function groupIdsBySourceFolder(ids, files, activeFolderId) {
    var fileMap = new Map(files.map(function (f) { return [f.id, f]; }));
    var groups = new Map();
    ids.forEach(function (id) {
      var file = fileMap.get(id) || {};
      var source =
        file.folder_id !== undefined && file.folder_id !== null
          ? file.folder_id
          : activeFolderId;
      var key = source === null ? '__home__' : String(source);
      if (!groups.has(key)) {
        groups.set(key, { sourceFolderId: source, ids: [] });
      }
      groups.get(key).ids.push(id);
    });
    return groups;
  }

  function buildBulkDeletePayloads(ids, files) {
    var groups = groupIdsBySourceFolder(ids, files, null);
    var payloads = [];
    groups.forEach(function (entry) {
      var body = { action: 'delete', file_ids: entry.ids };
      if (entry.sourceFolderId != null) body.folder_id = entry.sourceFolderId;
      payloads.push(body);
    });
    return payloads;
  }

  function buildBulkMovePayloads(ids, files, targetFolderId) {
    var groups = groupIdsBySourceFolder(ids, files, null);
    var payloads = [];
    groups.forEach(function (entry) {
      if (entry.sourceFolderId === targetFolderId) return;
      var body = {
        action: 'move',
        file_ids: entry.ids,
        payload: { folder_id: targetFolderId },
      };
      if (entry.sourceFolderId != null) body.folder_id = entry.sourceFolderId;
      payloads.push(body);
    });
    return payloads;
  }

  function buildFileDownloadUrl(id, folderId) {
    var url = '/api/v1/files/' + encodeURIComponent(String(id)) + '/download';
    if (folderId != null) url += '?folder_id=' + encodeURIComponent(String(folderId));
    return url;
  }

  function canBulkMoveInTransportMode(transportMode) {
    return String(transportMode || '').toLowerCase() === 'user';
  }

  function bulkMoveBlockedMessage(transportMode, surface) {
    if (canBulkMoveInTransportMode(transportMode)) return '';
    if (surface === 'desktop') {
      return '批量移动需要 User 模式 — 请在设置中切换传输模式。';
    }
    return 'Bulk move requires User mode — switch transport in Settings or use the desktop app.';
  }

  function resolveBulkBatchSucceededIds(fileIds, reportedCount) {
    var expected = fileIds.length;
    var count = Math.max(0, reportedCount || 0);
    if (count === 0) {
      return { succeededIds: [], partialBatch: false };
    }
    if (count === expected) {
      return { succeededIds: fileIds.slice(), partialBatch: false };
    }
    return { succeededIds: [], partialBatch: true };
  }

  function pickBulkSucceededIds(fileIds, reportedCount, apiSucceededIds) {
    if (Array.isArray(apiSucceededIds) && apiSucceededIds.length > 0) {
      return { succeededIds: apiSucceededIds.slice(), partialBatch: false };
    }
    return resolveBulkBatchSucceededIds(fileIds, reportedCount);
  }

  function buildTelegramLoginUrl(webBaseUrl, nextPath) {
    var base = webBaseUrl.replace(/\/$/, '');
    return base + '/telegram.html?next=' + encodeURIComponent(nextPath);
  }

  global.TdFilesPure = {
    buildBulkDeletePayloads: buildBulkDeletePayloads,
    buildBulkMovePayloads: buildBulkMovePayloads,
    buildFileDownloadUrl: buildFileDownloadUrl,
    buildTelegramLoginUrl: buildTelegramLoginUrl,
    canBulkMoveInTransportMode: canBulkMoveInTransportMode,
    bulkMoveBlockedMessage: bulkMoveBlockedMessage,
    resolveBulkBatchSucceededIds: resolveBulkBatchSucceededIds,
    pickBulkSucceededIds: pickBulkSucceededIds,
  };
})(typeof window !== 'undefined' ? window : globalThis);
