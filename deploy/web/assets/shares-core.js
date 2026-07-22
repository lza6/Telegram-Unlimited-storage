(function () {
  if (!TdApi.requireLogin()) return;

  TdApi.initSidebar('shares');

  var listEl = document.getElementById('shares-list');
  var refreshBtn = document.getElementById('refresh-shares');
  var domainInput = document.getElementById('share-domain');
  var createForm = document.getElementById('create-share-form');
  var serviceBanner = document.getElementById('service-banner');
  var serviceReady = true;
  var serviceHint = '';

  async function refreshServiceStatus() {
    try {
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
      if (serviceBanner) {
        serviceBanner.hidden = false;
        serviceBanner.textContent = '服务未就绪：' + serviceHint;
      }
    }
    syncCreateFormState();
  }

  function syncCreateFormState() {
    if (!createForm) return;
    var submitBtn = createForm.querySelector('button[type="submit"]');
    createForm.querySelectorAll('input').forEach(function (el) {
      el.disabled = !serviceReady;
    });
    if (submitBtn) {
      submitBtn.disabled = !serviceReady;
      submitBtn.title = serviceReady ? '' : '服务未就绪：' + serviceHint;
    }
  }

  async function ensureReadyForAction() {
    await refreshServiceStatus();
    if (!serviceReady) {
      TdApi.showToast('服务未就绪：' + serviceHint, 'err');
      throw new Error(serviceHint);
    }
  }

  TdShareDomain.loadShareDomainFromServer().then(function (domain) {
    domainInput.value = domain;
  });

  domainInput.addEventListener('change', function () {
    TdShareDomain.saveShareDomainToServer(domainInput.value)
      .then(function () {
        TdApi.showToast('分享域名已保存');
      })
      .catch(function (e) {
        TdApi.showToast(String(e.message || e), 'err');
      });
  });

  function displayLink(link) {
    return TdShareDomain.applyShareDomain(link, domainInput.value.trim());
  }

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/"/g, '&quot;');
  }

  var createShareInFlight = false;

  if (createForm) {
    createForm.addEventListener('submit', async function (ev) {
      ev.preventDefault();
      if (createShareInFlight) return;
      var messageId = parseInt(document.getElementById('share-message-id').value, 10);
      var fileName = document.getElementById('share-file-name').value.trim();
      var fileSize = parseInt(document.getElementById('share-file-size').value, 10) || 0;
      var folderRaw = document.getElementById('share-folder-id').value.trim();
      var folderId = folderRaw ? parseInt(folderRaw, 10) : null;
      var passwordRaw = document.getElementById('share-password').value;
      var password = passwordRaw.trim() ? passwordRaw : null;
      var expiryRaw = document.getElementById('share-expiry-hours').value.trim();
      var expiryHours = expiryRaw ? parseInt(expiryRaw, 10) : null;

      if (expiryRaw && (!Number.isFinite(expiryHours) || expiryHours <= 0)) {
        TdApi.showToast('有效期必须为正整数（小时）', 'err');
        return;
      }
      if (folderRaw && (!Number.isFinite(folderId) || folderId <= 0)) {
        TdApi.showToast('folder_id 必须为正整数', 'err');
        return;
      }
      if (!Number.isFinite(messageId) || messageId <= 0 || !fileName) {
        TdApi.showToast('请填写有效的 message_id 与文件名', 'err');
        return;
      }

      try {
        await ensureReadyForAction();
      } catch (e) {
        return;
      }

      createShareInFlight = true;
      var submitButton = createForm.querySelector('button[type=submit]');
      if (submitButton) submitButton.disabled = true;
      try {
        var info = await TdApi.apiJson('/api/v1/shares', {
          method: 'POST',
          body: {
            message_id: messageId,
            file_name: fileName,
            file_size: fileSize,
            folder_id: folderId,
            password: password,
            expiry_hours: expiryHours,
          },
        });
        var link = displayLink(info.link);
        await TdApi.copyToClipboard(link, '分享已创建，链接已复制');
        createForm.reset();
        loadShares();
      } catch (e) {
        TdApi.showToast(String(e.message || e), 'err');
      } finally {
        createShareInFlight = false;
        if (submitButton) submitButton.disabled = false;
      }
    });
  }

  async function loadShares() {
    listEl.innerHTML = '<p class="muted">加载中…</p>';
    try {
      var shares = await TdApi.apiJson('/api/v1/shares');
      if (!shares.length) {
        listEl.innerHTML = '<p class="muted">暂无分享链接</p>';
        return;
      }
      listEl.innerHTML = shares
        .map(function (s) {
          var expired = s.expires_at && s.expires_at < Math.floor(Date.now() / 1000);
          var link = displayLink(s.link);
          return (
            '<div class="share-card">' +
            '<div class="share-card-head">' +
            '<strong title="' +
            escapeHtml(s.file_name) +
            '">' +
            escapeHtml(s.file_name) +
            '</strong>' +
            '<span class="tag ' +
            (s.has_password ? 'tag-ok' : '') +
            '">' +
            (s.has_password ? '有密码' : '公开') +
            '</span>' +
            (expired ? '<span class="tag tag-err">已过期</span>' : '') +
            '</div>' +
            '<div class="muted share-meta">' +
            TdApi.formatSize(s.file_size) +
            ' · ' +
            new Date(s.created_at * 1000).toLocaleString() +
            '</div>' +
            '<code class="share-link">' +
            escapeHtml(link) +
            '</code>' +
            '<div class="share-actions">' +
            '<button type="button" class="btn-secondary btn-sm" data-copy="' +
            escapeHtml(link) +
            '">复制</button>' +
            '<button type="button" class="btn-danger btn-sm" data-revoke="' +
            escapeHtml(s.id) +
            '">撤销</button>' +
            '</div></div>'
          );
        })
        .join('');

      listEl.querySelectorAll('[data-copy]').forEach(function (btn) {
        btn.addEventListener('click', function () {
          TdApi.copyToClipboard(btn.getAttribute('data-copy') || '', '已复制');
        });
      });

      listEl.querySelectorAll('[data-revoke]').forEach(function (btn) {
        btn.addEventListener('click', async function () {
          var id = btn.getAttribute('data-revoke');
          if (!confirm('撤销分享链接 ' + id + '？')) return;
          try {
            await ensureReadyForAction();
          } catch (e) {
            return;
          }
          try {
            await TdApi.apiJson('/api/v1/shares/' + encodeURIComponent(id), {
              method: 'DELETE',
            });
            TdApi.showToast('已撤销');
            loadShares();
          } catch (e) {
            TdApi.showToast(String(e.message || e), 'err');
          }
        });
      });
    } catch (e) {
      listEl.innerHTML = '<p class="err">' + escapeHtml(e.message || e) + '</p>';
      TdApi.showToast(String(e.message || e), 'err');
    }
  }

  // File deletion can happen in another browser tab. Reconcile the list on
  // storage/custom-event notifications and when the tab becomes visible again
  // so revoked links never remain stale until a manual refresh.
  window.addEventListener('storage', function (event) {
    if (event.key === 'td-shares-invalidate') loadShares();
  });
  window.addEventListener('td-shares-invalidate', loadShares);
  document.addEventListener('visibilitychange', function () {
    if (!document.hidden) loadShares();
  });

  refreshBtn.addEventListener('click', loadShares);
  loadShares();
  refreshServiceStatus();
  setInterval(refreshServiceStatus, 30000);
})();
