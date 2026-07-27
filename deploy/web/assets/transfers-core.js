/**
 * Telegram Drive Transfers Center (TASK-U-02).
 *
 * Real-time queue table + floating panel for active transfers.
 * Polls /api/v1/transfers (or SSE /api/v1/transfers/events).
 */
(function (global) {
  'use strict';

  function getAccessPwd() {
    return sessionStorage.getItem('td_access_pwd') || sessionStorage.getItem('pwd') || '';
  }

  function formatSize(bytes) {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1048576) return (bytes / 1024).toFixed(2) + ' KB';
    if (bytes < 1073741824) return (bytes / 1048576).toFixed(2) + ' MB';
    return (bytes / 1073741824).toFixed(2) + ' GB';
  }

  function formatTime(seconds) {
    if (seconds <= 0 || !Number.isFinite(seconds)) return '—';
    var hrs = Math.floor(seconds / 3600);
    var mins = Math.floor((seconds % 3600) / 60);
    var secs = Math.floor(seconds % 60);
    if (hrs > 0) return hrs + 'h' + mins + 'm';
    if (mins > 0) return mins + 'm' + secs + 's';
    return secs + 's';
  }

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  var STATUS_LABELS = {
    queued: '排队中',
    running: '进行中',
    retry_wait: '等待重试',
    completed: '已完成',
    failed: '失败',
    cancelled: '已取消',
    paused: '已暂停',
  };

  function renderRow(t) {
    var pct = t.total_chunks > 0 ? Math.round((t.uploaded_chunks / t.total_chunks) * 100) : 0;
    var statusLabel = STATUS_LABELS[t.status] || t.status;
    var speed = t.speed > 0 ? formatSize(t.speed) + '/s' : '—';
    var eta = t.eta > 0 ? formatTime(t.eta) : '—';
    return '<tr data-session="' + escapeHtml(t.session_id) + '">' +
      '<td>' + escapeHtml(t.filename || '—') + '</td>' +
      '<td>' + formatSize(t.total_size || 0) + '</td>' +
      '<td><progress value="' + pct + '" max="100" aria-label="' + escapeHtml(t.filename || '') + ' 进度"></progress> ' + pct + '%</td>' +
      '<td>' + speed + '</td>' +
      '<td>' + eta + '</td>' +
      '<td class="status-' + escapeHtml(t.status) + '">' + statusLabel + '</td>' +
      '<td>' +
        (t.status === 'running' || t.status === 'queued'
          ? '<button class="btn-cancel" data-action="cancel" data-id="' + escapeHtml(t.session_id) + '">取消</button>'
          : '') +
        (t.status === 'failed' || t.status === 'cancelled'
          ? '<button class="btn-retry" data-action="retry" data-id="' + escapeHtml(t.session_id) + '">重试</button>'
          : '') +
      '</td>' +
    '</tr>';
  }

  function renderTable(transfers) {
    var tbody = document.getElementById('transfers-tbody');
    if (!tbody) return;
    if (!transfers || transfers.length === 0) {
      tbody.innerHTML = '<tr><td colspan="7" class="empty">暂无活跃传输</td></tr>';
      return;
    }
    tbody.innerHTML = transfers.map(renderRow).join('');
  }

  function updateFloatingPanel(transfers) {
    var panel = document.getElementById('td-transfers-fab');
    if (!panel) return;
    var active = transfers.filter(function (t) {
      return t.status === 'running' || t.status === 'queued';
    });
    var count = active.length;
    var totalPct = 0;
    active.forEach(function (t) {
      if (t.total_chunks > 0) {
        totalPct += (t.uploaded_chunks / t.total_chunks) * 100;
      }
    });
    var avgPct = count > 0 ? Math.round(totalPct / count) : 0;
    panel.querySelector('.fab-count').textContent = String(count);
    panel.querySelector('.fab-label').textContent = count > 0 ? count + ' 个传输 · ' + avgPct + '%' : '无活跃传输';
    panel.hidden = false;
  }

  async function fetchTransfers() {
    var pwd = getAccessPwd();
    if (!pwd) return [];
    try {
      var res = await fetch('/api/v1/transfers', { headers: { 'X-Access-Pwd': pwd } });
      if (!res.ok) return [];
      var data = await res.json();
      return data.transfers || [];
    } catch (e) {
      console.warn('fetch transfers failed', e);
      return [];
    }
  }

  async function actionTransfer(sessionId, action) {
    var pwd = getAccessPwd();
    if (!pwd) return;
    try {
      await fetch('/api/v1/transfers/' + encodeURIComponent(sessionId) + '/' + action, {
        method: 'POST',
        headers: { 'X-Access-Pwd': pwd },
      });
    } catch (e) {
      console.warn('transfer action failed', e);
    }
    await refresh();
  }

  var pollTimer = null;
  var eventSource = null;

  async function refresh() {
    var transfers = await fetchTransfers();
    renderTable(transfers);
    updateFloatingPanel(transfers);
  }

  function startPolling() {
    if (pollTimer) return;
    refresh();
    pollTimer = setInterval(refresh, 2000);
  }

  function stopPolling() {
    if (pollTimer) { clearInterval(pollTimer); pollTimer = null; }
    if (eventSource) { eventSource.close(); eventSource = null; }
  }

  function startSSE() {
    var pwd = getAccessPwd();
    if (!pwd || typeof EventSource === 'undefined') {
      startPolling();
      return;
    }
    try {
      eventSource = new EventSource('/api/v1/transfers/events?pwd=' + encodeURIComponent(pwd));
      eventSource.onmessage = function (ev) {
        try {
          var data = JSON.parse(ev.data);
          if (Array.isArray(data)) {
            renderTable(data);
            updateFloatingPanel(data);
          }
        } catch (e) { /* ignore */ }
      };
      eventSource.onerror = function () {
        eventSource.close();
        eventSource = null;
        // Fallback to polling
        startPolling();
      };
    } catch (e) {
      startPolling();
    }
  }

  function initTransfersCenter(options) {
    options = options || {};
    var table = document.querySelector(options.tableSelector || '#transfers-table');
    if (table) {
      table.addEventListener('click', function (ev) {
        var btn = ev.target.closest('[data-action]');
        if (!btn) return;
        var action = btn.getAttribute('data-action');
        var id = btn.getAttribute('data-id');
        if (action && id) actionTransfer(id, action);
      });
    }
    // Start with SSE, fall back to polling
    startSSE();
    // Visibility-based pause
    document.addEventListener('visibilitychange', function () {
      if (document.hidden) {
        stopPolling();
        if (eventSource) { eventSource.close(); eventSource = null; }
      } else {
        startSSE();
      }
    });
  }

  function initFloatingPanel() {
    var fab = document.getElementById('td-transfers-fab');
    if (!fab) return;
    fab.addEventListener('click', function () {
      location.href = '/transfers.html';
    });
  }

  global.TdTransfers = {
    init: initTransfersCenter,
    initFloatingPanel: initFloatingPanel,
    refresh: refresh,
  };

  document.addEventListener('DOMContentLoaded', function () {
    initFloatingPanel();
    if (document.getElementById('transfers-table')) {
      initTransfersCenter({});
    } else {
      // On non-transfers pages, still poll for the floating panel
      startPolling();
    }
  });
})(window);
