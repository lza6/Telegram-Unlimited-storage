(function () {
  var fileById = new Map();

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/"/g, '&quot;');
  }

  if (!TdApi.requireLogin()) return;

  TdApi.initSidebar('files');

  var state = { page: 1, limit: 25, search: '', selected: new Set() };
  /** Persists file metadata for cross-page bulk delete (fileById clears on each render). */
  var selectedMeta = new Map();

  function rememberFileMeta(f) {
    if (!f) return;
    var id = String(f.id);
    selectedMeta.set(id, {
      id: f.id,
      name: f.name || f.filename,
      filename: f.filename,
      size: f.size,
      folder_id: f.folder_id != null ? f.folder_id : null,
      mime_type: f.mime_type,
    });
  }

  function forgetFileMeta(id) {
    selectedMeta.delete(String(id));
  }

  function clearSelection() {
    state.selected.clear();
    selectedMeta.clear();
    if (selectAll) selectAll.checked = false;
  }

  var tbody = document.getElementById('files-body');
  var pager = document.getElementById('pager');
  var searchInput = document.getElementById('search-input');
  var refreshBtn = document.getElementById('refresh-btn');
  var deleteBtn = document.getElementById('bulk-delete-btn');
  var moveBtn = document.getElementById('bulk-move-btn');
  var moveFolderSelect = document.getElementById('bulk-move-folder');
  var selectAll = document.getElementById('select-all');

  var serviceBanner = document.getElementById('service-banner');
  var serviceReady = true;
  var serviceHint = '';
  var transportMode = 'user';
  var downloadingIds = new Set();

  async function refreshServiceStatus() {
    try {
      var hv = await TdApi.fetchHealth();
      transportMode = hv.transport_mode || 'user';
      await TdApi.ensureServiceReady();
      serviceReady = true;
      serviceHint = '';
      if (serviceBanner) {
        serviceBanner.hidden = true;
        serviceBanner.textContent = '';
      }
    } catch (e) {
      serviceReady = false;
      serviceHint = String(e.message || e);
      try {
        var hvFail = await TdApi.fetchHealth();
        transportMode = hvFail.transport_mode || 'bot';
      } catch (ignore) {
        transportMode = 'bot';
      }
      if (serviceBanner) {
        serviceBanner.hidden = false;
        serviceBanner.textContent = '服务未就绪：' + serviceHint;
      }
    }
    updateBulkUi();
    tbody.querySelectorAll('.btn-dl, .btn-share').forEach(function (btn) {
      btn.disabled = !serviceReady;
      btn.title = serviceReady ? '' : '服务未就绪';
    });
  }

  async function ensureReadyForAction() {
    await refreshServiceStatus();
    if (!serviceReady) {
      TdApi.showToast('服务未就绪：' + serviceHint, 'err');
      throw new Error(serviceHint);
    }
  }

  function renderRows(files) {
    fileById.clear();
    if (!files.length) {
      var emptyMsg = state.search.trim()
        ? '未找到匹配「' + escapeHtml(state.search.trim()) + '」的文件，请尝试更短或不同的关键词'
        : '暂无文件（Bot 模式请先上传；User 模式需 Telegram 已连接）';
      tbody.innerHTML = '<tr><td colspan="7" class="muted center">' + emptyMsg + '</td></tr>';
      return;
    }
    tbody.innerHTML = files
      .map(function (f) {
        var id = String(f.id);
        fileById.set(id, f);
        if (state.selected.has(id)) {
          rememberFileMeta(f);
        }
        var checked = state.selected.has(id) ? ' checked' : '';
        var name = f.name || f.filename || '—';
        var actionDisabled = serviceReady ? '' : ' disabled';
        var actionTitle = serviceReady ? '' : ' title="服务未就绪"';
        return (
          '<tr>' +
          '<td><input type="checkbox" class="row-check" data-id="' +
          id +
          '"' +
          checked +
          ' /></td>' +
          '<td><code>' +
          escapeHtml(id) +
          '</code></td>' +
          '<td class="name-cell" title="' +
          escapeHtml(name) +
          '">' +
          escapeHtml(name) +
          '</td>' +
          '<td>' +
          TdApi.formatSize(f.size) +
          '</td>' +
          '<td class="muted">' +
          escapeHtml(f.mime_type || '—') +
          '</td>' +
          '<td class="muted">' +
          escapeHtml(f.created_at || '—') +
          '</td>' +
          '<td class="row-actions">' +
          '<button type="button" class="btn-secondary btn-sm btn-dl" data-id="' +
          id +
          '"' +
          actionDisabled +
          actionTitle +
          '>下载</button> ' +
          '<button type="button" class="btn-secondary btn-sm btn-share" data-id="' +
          id +
          '"' +
          actionDisabled +
          actionTitle +
          '>分享</button>' +
          '</td>' +
          '</tr>'
        );
      })
      .join('');

    tbody.querySelectorAll('.row-check').forEach(function (cb) {
      cb.addEventListener('change', function () {
        var id = cb.getAttribute('data-id');
        if (cb.checked) {
          state.selected.add(id);
          rememberFileMeta(fileById.get(id));
        } else {
          state.selected.delete(id);
          forgetFileMeta(id);
        }
        updateBulkUi();
      });
    });

    tbody.querySelectorAll('.btn-dl').forEach(function (btn) {
      btn.addEventListener('click', function () {
        if (btn.disabled) {
          TdApi.showToast('服务未就绪：' + serviceHint, 'err');
          return;
        }
        downloadFile(btn.getAttribute('data-id'));
      });
    });

    tbody.querySelectorAll('.btn-share').forEach(function (btn) {
      btn.addEventListener('click', function () {
        if (btn.disabled) {
          TdApi.showToast('服务未就绪：' + serviceHint, 'err');
          return;
        }
        createShare(btn.getAttribute('data-id'));
      });
    });
  }

  async function downloadFile(id) {
    var sid = String(id);
    if (TdDownloadPure.shouldBlockDuplicateDownload(downloadingIds, sid)) {
      TdApi.showToast('该文件正在下载，请稍候', 'info');
      return;
    }
    var f = fileById.get(sid);
    if (!f) {
      TdApi.showToast('文件信息不可用，请刷新后重试', 'err');
      return;
    }
    try {
      await ensureReadyForAction();
    } catch (e) {
      return;
    }
    var url = TdFilesPure.buildFileDownloadUrl(id, f.folder_id);
    var dlBtn = tbody.querySelector('.btn-dl[data-id="' + sid + '"]');
    downloadingIds.add(sid);
    var btnState = TdDownloadPure.deriveWebDownloadButtonState(true);
    if (dlBtn) {
      dlBtn.disabled = true;
      dlBtn.textContent = btnState.label;
    }
    TdApi.showToast(TdDownloadPure.buildDownloadStartToast(f));
    try {
      var res = await TdApi.apiFetch(url);
      if (!res.ok) throw new Error(await res.text());
      var blob = await TdDownloadPure.readResponseBlobWithProgress(res, function (pct) {
        if (!dlBtn) return;
        var st = TdDownloadPure.deriveWebDownloadButtonState(true, pct);
        dlBtn.textContent = st.label;
      });
      var a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = TdDownloadPure.resolveBlobDownloadFilename(f);
      a.click();
      URL.revokeObjectURL(a.href);
      TdApi.showToast('下载已开始');
    } catch (e) {
      TdApi.showToast(String(e.message || e), 'err');
    } finally {
      downloadingIds.delete(sid);
      if (dlBtn) {
        var idle = TdDownloadPure.deriveWebDownloadButtonState(false);
        dlBtn.disabled = !serviceReady;
        dlBtn.textContent = idle.label;
      }
    }
  }

  async function createShare(id) {
    var f = fileById.get(String(id));
    if (!f) {
      TdApi.showToast('文件信息不可用，请刷新后重试', 'err');
      return;
    }
    try {
      await ensureReadyForAction();
    } catch (e) {
      return;
    }
    if (
      !confirm(
        '将创建无密码、永久有效的公开分享链接并复制到剪贴板。需要密码或有效期请前往「分享管理」页面创建。',
      )
    ) {
      return;
    }
    try {
      var info = await TdApi.apiJson('/api/v1/shares', {
        method: 'POST',
        body: {
          message_id: parseInt(String(f.id), 10),
          file_name: f.name || f.filename || 'file',
          file_size: f.size || 0,
          folder_id: f.folder_id != null ? f.folder_id : null,
        },
      });
      var link = TdShareDomain.applyShareDomain(info.link);
      await TdApi.copyToClipboard(link, '分享链接已复制');
    } catch (e) {
      TdApi.showToast(String(e.message || e), 'err');
    }
  }

  function updateBulkUi() {
    var hasSelection = state.selected.size > 0;
    var moveAllowed = serviceReady && TdFilesPure.canBulkMoveInTransportMode(transportMode);
    deleteBtn.disabled = !hasSelection;
    deleteBtn.textContent = hasSelection
      ? '删除选中 (' + state.selected.size + ')'
      : '删除选中';
    deleteBtn.title = !serviceReady && hasSelection ? '服务未就绪：' + serviceHint : '';
    if (moveBtn) {
      moveBtn.disabled = !hasSelection;
      moveBtn.textContent = hasSelection
        ? '移动选中 (' + state.selected.size + ')'
        : '移动选中';
      moveBtn.title = !TdFilesPure.canBulkMoveInTransportMode(transportMode)
        ? TdFilesPure.bulkMoveBlockedMessage(transportMode)
        : serviceReady
          ? ''
          : '服务未就绪：' + serviceHint;
    }
    if (moveFolderSelect) {
      moveFolderSelect.disabled = !moveAllowed;
      moveFolderSelect.title = moveBtn ? moveBtn.title : '';
    }
    if (refreshBtn) {
      refreshBtn.title = serviceReady ? '' : '服务未就绪：' + serviceHint;
    }
  }

  async function loadMoveFolders() {
    if (!moveFolderSelect) return;
    try {
      var folders = await TdApi.apiJson('/api/v1/folders');
      moveFolderSelect.innerHTML = '<option value="">Saved Messages</option>';
      (folders || []).forEach(function (f) {
        var opt = document.createElement('option');
        opt.value = String(f.id);
        opt.textContent = f.name || ('Folder ' + f.id);
        moveFolderSelect.appendChild(opt);
      });
    } catch (e) {
      TdApi.showToast('加载文件夹列表失败：' + String(e.message || e), 'err');
    }
  }

  function renderPager(pagination) {
    if (!pagination) {
      pager.textContent = '';
      return;
    }
    pager.innerHTML =
      '<span class="muted">共 ' +
      pagination.total +
      ' 条 · 第 ' +
      pagination.page +
      '/' +
      pagination.total_pages +
      ' 页</span>' +
      '<span class="pager-btns">' +
      '<button type="button" class="btn-secondary btn-sm" id="prev-page"' +
      (pagination.has_prev ? '' : ' disabled') +
      '>上一页</button>' +
      '<button type="button" class="btn-secondary btn-sm" id="next-page"' +
      (pagination.has_next ? '' : ' disabled') +
      '>下一页</button>' +
      '</span>';
    document.getElementById('prev-page')?.addEventListener('click', function () {
      if (state.page > 1) {
        state.page -= 1;
        loadFiles();
      }
    });
    document.getElementById('next-page')?.addEventListener('click', function () {
      state.page += 1;
      loadFiles();
    });
  }

  async function loadFiles() {
    tbody.innerHTML = '<tr><td colspan="7" class="muted center">加载中…</td></tr>';
    try {
      if (state.search.trim()) {
        var url = '/api/v1/files/search?q=' + encodeURIComponent(state.search.trim());
        var searchRes = await TdApi.apiJson(url);
        var list = Array.isArray(searchRes) ? searchRes : searchRes.data || searchRes.files || [];
        renderRows(list);
        renderPager(null);
        pager.innerHTML = '<span class="muted">搜索到 ' + list.length + ' 条</span>';
      } else {
        var url =
          '/api/v1/files?page=' +
          state.page +
          '&limit=' +
          state.limit;
        var res = await TdApi.apiJson(url);
        var files = res.files || res.data || [];
        renderRows(files);
        renderPager(res.pagination);
      }
      updateBulkUi();
    } catch (e) {
      tbody.innerHTML =
        '<tr><td colspan="7" class="err center">' + escapeHtml(e.message || e) + '</td></tr>';
      TdApi.showToast(String(e.message || e), 'err');
    }
  }

  searchInput.addEventListener('keydown', async function (ev) {
    if (ev.key === 'Enter') {
      state.page = 1;
      state.search = searchInput.value;
      clearSelection();
      if (state.search.trim()) {
        await rebuildIndexIfUser('search');
      }
      loadFiles();
    }
  });

  async function rebuildIndexIfUser(trigger) {
    trigger = trigger || 'refresh';
    try {
      var hv = await TdApi.fetchHealth();
      if (hv.transport_mode !== 'user' || !hv.ready) return;
      var folders = await TdApi.apiJson('/api/v1/folders');
      var folderIds = [null].concat(
        (folders || []).map(function (f) {
          return f.id;
        }),
      );
      var rebuilt = await TdApi.apiJson('/api/v1/files/rebuild-index', {
        method: 'POST',
        body: { folder_ids: folderIds },
      });
      if (
        rebuilt &&
        rebuilt.files_indexed != null &&
        typeof TdWebPure !== 'undefined' &&
        TdWebPure.rebuildIndexShouldToast(trigger)
      ) {
        TdApi.showToast(
          TdWebPure.formatRebuildIndexSuccessToast(
            rebuilt.files_indexed,
            rebuilt.folders_scanned,
          ),
        );
      }
    } catch (e) {
      if (
        typeof TdWebPure !== 'undefined' &&
        TdWebPure.rebuildIndexShouldSurfaceBackgroundFailure(trigger)
      ) {
        TdApi.showToast(TdWebPure.formatRebuildIndexBackgroundFailureMessage(e));
      }
    }
  }

  refreshBtn.addEventListener('click', async function () {
    refreshBtn.disabled = true;
    try {
      await rebuildIndexIfUser('refresh');
      await loadFiles();
    } finally {
      refreshBtn.disabled = false;
    }
  });

  selectAll.addEventListener('change', function () {
    var checks = tbody.querySelectorAll('.row-check');
    checks.forEach(function (cb) {
      cb.checked = selectAll.checked;
      var id = cb.getAttribute('data-id');
      if (selectAll.checked) {
        state.selected.add(id);
        rememberFileMeta(fileById.get(id));
      } else {
        state.selected.delete(id);
        forgetFileMeta(id);
      }
    });
    updateBulkUi();
  });

  async function bulkDeleteByFolder(ids) {
    var files = ids.map(function (id) {
      return (
        selectedMeta.get(String(id)) ||
        fileById.get(String(id)) ||
        { id: id, folder_id: null }
      );
    });
    var payloads = TdFilesPure.buildBulkDeletePayloads(ids, files);
    var total = 0;
    var failures = [];
    var succeededIds = [];
    var partialBatches = 0;
    for (var i = 0; i < payloads.length; i++) {
      try {
        var res = await TdApi.apiJson('/api/v1/files/bulk', {
          method: 'POST',
          body: payloads[i],
        });
        var batchCount = res.count || 0;
        total += batchCount;
        var batchResult = TdFilesPure.pickBulkSucceededIds(
          payloads[i].file_ids,
          batchCount,
          res.succeeded_ids,
        );
        if (batchResult.partialBatch) partialBatches += 1;
        batchResult.succeededIds.forEach(function (id) {
          succeededIds.push(id);
        });
      } catch (e) {
        failures.push(String(e.message || e));
      }
    }
    if (partialBatches > 0) {
      TdApi.showToast(
        '部分条目未能全部处理（' +
          partialBatches +
          ' 批），已保留未确认的选中项，请刷新后重试',
        'err',
      );
    }
    if (failures.length) {
      TdApi.showToast(
        failures.length === payloads.length
          ? failures[0]
          : '部分删除失败（已成功 ' + total + ' 条）: ' + failures[0],
        'err',
      );
    }
    return { total: total, failures: failures, succeededIds: succeededIds };
  }

  function parseMoveTargetFolderId(raw) {
    if (raw == null || raw === '') return null;
    var parsed = parseInt(String(raw), 10);
    if (!Number.isFinite(parsed)) return NaN;
    return parsed;
  }

  async function bulkMoveByFolder(ids, targetFolderId) {
    var files = ids.map(function (id) {
      return (
        selectedMeta.get(String(id)) ||
        fileById.get(String(id)) ||
        { id: id, folder_id: null }
      );
    });
    var payloads = TdFilesPure.buildBulkMovePayloads(ids, files, targetFolderId);
    if (!payloads.length) {
      return { total: 0, failures: [], succeededIds: [] };
    }
    var total = 0;
    var failures = [];
    var succeededIds = [];
    var partialBatches = 0;
    for (var i = 0; i < payloads.length; i++) {
      try {
        var res = await TdApi.apiJson('/api/v1/files/bulk', {
          method: 'POST',
          body: payloads[i],
        });
        var batchCount = res.count || 0;
        total += batchCount;
        var batchResult = TdFilesPure.pickBulkSucceededIds(
          payloads[i].file_ids,
          batchCount,
          res.succeeded_ids,
        );
        if (batchResult.partialBatch) partialBatches += 1;
        batchResult.succeededIds.forEach(function (id) {
          succeededIds.push(id);
        });
      } catch (e) {
        failures.push(String(e.message || e));
      }
    }
    if (partialBatches > 0) {
      TdApi.showToast(
        '部分条目未能全部处理（' +
          partialBatches +
          ' 批），已保留未确认的选中项，请刷新后重试',
        'err',
      );
    }
    if (failures.length) {
      TdApi.showToast(
        failures.length === payloads.length
          ? failures[0]
          : '部分移动失败（' + failures.length + '/' + payloads.length + '）：' + failures[0],
        'err',
      );
    }
    return { total: total, failures: failures, succeededIds: succeededIds };
  }

  if (moveBtn) {
    moveBtn.addEventListener('click', async function () {
      if (!state.selected.size) return;
      if (!TdFilesPure.canBulkMoveInTransportMode(transportMode)) {
        TdApi.showToast(TdFilesPure.bulkMoveBlockedMessage(transportMode), 'err');
        return;
      }
      var targetFolderId = parseMoveTargetFolderId(
        moveFolderSelect ? moveFolderSelect.value : '',
      );
      if (Number.isNaN(targetFolderId)) {
        TdApi.showToast('请选择有效的目标文件夹', 'err');
        return;
      }
      var targetLabel =
        targetFolderId == null
          ? 'Saved Messages'
          : moveFolderSelect && moveFolderSelect.selectedOptions[0]
            ? moveFolderSelect.selectedOptions[0].textContent
            : '目标文件夹';
      if (
        !confirm(
          '确定将选中的 ' +
            state.selected.size +
            ' 个文件移动到「' +
            targetLabel +
            '」？User 模式下将转发 Telegram 消息。',
        )
      ) {
        return;
      }
      try {
        await ensureReadyForAction();
      } catch (e) {
        return;
      }
      moveBtn.disabled = true;
      try {
        var ids = Array.from(state.selected).map(function (s) {
          return parseInt(s, 10);
        });
        var moveResult = await bulkMoveByFolder(ids, targetFolderId);
        if (moveResult.total > 0) {
          TdApi.showToast('已移动 ' + moveResult.total + ' 个文件');
          moveResult.succeededIds.forEach(function (id) {
            state.selected.delete(String(id));
            forgetFileMeta(id);
          });
          updateBulkUi();
          loadFiles();
        } else if (!moveResult.failures.length) {
          TdApi.showToast('所选文件已在目标文件夹', 'info');
        }
      } catch (e) {
        TdApi.showToast(String(e.message || e), 'err');
      } finally {
        updateBulkUi();
      }
    });
  }

  deleteBtn.addEventListener('click', async function () {
    if (!state.selected.size) return;
    var hv;
    try {
      hv = await TdApi.fetchHealth();
    } catch (e) {
      hv = { transport_mode: 'bot' };
    }
    var confirmMsg =
      hv.transport_mode === 'user'
        ? '确定删除选中的 ' + state.selected.size + ' 个文件？User 模式下将同时删除 Telegram 消息。'
        : '确定删除选中的 ' + state.selected.size + ' 条索引？Bot 模式下 Telegram 消息不会被删除。';
    if (!confirm(confirmMsg)) return;
    try {
      await ensureReadyForAction();
    } catch (e) {
      return;
    }
    deleteBtn.disabled = true;
    try {
      var ids = Array.from(state.selected).map(function (s) {
        return parseInt(s, 10);
      });
      var deleteResult = await bulkDeleteByFolder(ids);
      if (deleteResult.total > 0) {
        deleteResult.succeededIds.forEach(function (id) {
          forgetFileMeta(id);
          state.selected.delete(String(id));
        });
        TdApi.showToast('已删除 ' + deleteResult.total + ' 条');
        updateBulkUi();
        loadFiles();
      } else if (!deleteResult.failures.length) {
        TdApi.showToast('没有可删除的条目', 'info');
      }
    } catch (e) {
      TdApi.showToast(String(e.message || e), 'err');
    } finally {
      updateBulkUi();
    }
  });

  loadFiles();
  loadMoveFolders();
  refreshServiceStatus();
  setInterval(refreshServiceStatus, 30000);
  TdShareDomain.loadShareDomainFromServer();
})();
