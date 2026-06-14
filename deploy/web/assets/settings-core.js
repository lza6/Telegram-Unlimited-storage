(function () {

  if (!TdApi.requireLogin()) return;



  TdApi.initSidebar('settings');



  var transportEl = document.getElementById('transport-active');

  var modesEl = document.getElementById('transport-modes');

  var healthEl = document.getElementById('settings-health');

  var metricsLink = document.getElementById('metrics-link');

  var metricsHint = document.getElementById('metrics-hint');

  var shareDomainInput = document.getElementById('share-domain-input');

  var envBaseUrlEl = document.getElementById('env-base-url');

  var effectiveBaseUrlEl = document.getElementById('effective-base-url');
  var effectiveShareBaseUrlEl = document.getElementById('effective-share-base-url');
  var effectiveStreamBaseUrlEl = document.getElementById('effective-stream-base-url');

  var chunkInfoEl = document.getElementById('chunk-info');

  var saveDomainBtn = document.getElementById('save-share-domain');

  var vpnEnabledEl = document.getElementById('vpn-enabled');

  var proxyEnabledEl = document.getElementById('proxy-enabled');

  var proxyHostEl = document.getElementById('proxy-host');

  var proxyPortEl = document.getElementById('proxy-port');

  var proxyUserEl = document.getElementById('proxy-username');

  var proxyPassEl = document.getElementById('proxy-password');

  var saveNetworkBtn = document.getElementById('save-network');

  var rebuildIndexBtn = document.getElementById('rebuild-index-btn');



  async function rebuildFileIndexManual() {
    if (!rebuildIndexBtn) return;
    try {
      var hv = await TdApi.fetchHealth();
      if (hv.transport_mode !== 'user') {
        TdApi.showToast('仅 User 模式支持重建索引', 'err');
        return;
      }
      if (!hv.ready) {
        TdApi.showToast('Telegram 会话未就绪，请先完成登录', 'err');
        return;
      }
      rebuildIndexBtn.disabled = true;
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
        TdWebPure.rebuildIndexShouldToast('manual')
      ) {
        TdApi.showToast(
          TdWebPure.formatRebuildIndexSuccessToast(
            rebuilt.files_indexed,
            rebuilt.folders_scanned,
          ),
        );
      }
    } catch (e) {
      TdApi.showToast(String(e.message || e), 'err');
    } finally {
      rebuildIndexBtn.disabled = false;
    }
  }

  if (rebuildIndexBtn) {
    rebuildIndexBtn.addEventListener('click', rebuildFileIndexManual);
  }



  async function loadSettingsPanel() {

    try {

      var data = await TdApi.apiJson('/api/v1/settings');

      if (shareDomainInput) shareDomainInput.value = data.share_domain || '';

      if (envBaseUrlEl) envBaseUrlEl.textContent = data.env_base_url || '（未设置 BASE_URL）';

      if (effectiveBaseUrlEl) effectiveBaseUrlEl.textContent = data.effective_base_url || '—';
      if (effectiveShareBaseUrlEl) {
        effectiveShareBaseUrlEl.textContent =
          data.effective_share_link_base || data.effective_base_url || '—';
      }
      if (effectiveStreamBaseUrlEl) {
        effectiveStreamBaseUrlEl.textContent = data.effective_share_base_url || '—';
      }

      if (chunkInfoEl) {

        chunkInfoEl.textContent =

          '分片 ' +

          data.chunk_size_mb +

          'MB · 并发 ' +

          data.chunk_concurrent +

          ' · 文件并发 ' +

          data.files_concurrent +

          ' · 最大上传 ' +

          data.max_upload_size_mb +

          'MB · 流媒体端口 ' +

          data.stream_port;

      }

      TdShareDomain.setLocalShareDomain(data.share_domain || '');

    } catch (e) {
      if (chunkInfoEl) chunkInfoEl.textContent = '加载失败: ' + (e.message || e);
      if (shareDomainInput) shareDomainInput.value = '';
      if (effectiveBaseUrlEl) effectiveBaseUrlEl.textContent = '加载失败';
      if (effectiveShareBaseUrlEl) effectiveShareBaseUrlEl.textContent = '加载失败';
      if (effectiveStreamBaseUrlEl) effectiveStreamBaseUrlEl.textContent = '加载失败';
      TdApi.showToast('设置加载失败: ' + (e.message || e), 'err');
    }

  }



  if (saveDomainBtn) {

    saveDomainBtn.addEventListener('click', async function () {

      try {

        var saved = await TdShareDomain.saveShareDomainToServer(shareDomainInput.value);

        if (saved && saved.effective_share_link_base && effectiveShareBaseUrlEl) {
          effectiveShareBaseUrlEl.textContent = saved.effective_share_link_base;
        } else if (saved && saved.effective_share_base_url && effectiveShareBaseUrlEl) {
          effectiveShareBaseUrlEl.textContent = saved.effective_share_base_url;
        }

        await loadSettingsPanel();

        TdApi.showToast('分享域名已保存到服务端');

      } catch (e) {

        TdApi.showToast(String(e.message || e), 'err');

      }

    });

  }



  async function loadNetworkPanel() {

    if (!vpnEnabledEl && !proxyEnabledEl) return;

    try {

      var net = await TdApi.apiJson('/api/v1/network');

      if (proxyEnabledEl) proxyEnabledEl.checked = !!net.proxy.enabled;

      if (proxyHostEl) proxyHostEl.value = net.proxy.host || '';

      if (proxyPortEl) proxyPortEl.value = String(net.proxy.port || 1080);

      if (proxyUserEl) proxyUserEl.value = net.proxy.username || '';

      if (proxyPassEl) {
        proxyPassEl.value = '';
        proxyPassEl.placeholder = net.proxy.password_set ? '（已配置，留空不修改）' : '可选';
      }

      if (vpnEnabledEl) vpnEnabledEl.checked = !!net.vpn.enabled;

    } catch (e) {

      TdApi.showToast('网络配置加载失败: ' + (e.message || e), 'err');

    }

  }



  if (saveNetworkBtn) {

    saveNetworkBtn.addEventListener('click', async function () {

      try {
        if (proxyEnabledEl && proxyEnabledEl.checked && proxyHostEl && !proxyHostEl.value.trim()) {
          TdApi.showToast('启用代理时必须填写 SOCKS5 主机', 'err');
          return;
        }
        await TdApi.apiJson('/api/v1/network', {
          method: 'PUT',
          body: {
            proxy: {
              enabled: proxyEnabledEl ? proxyEnabledEl.checked : undefined,
              host: proxyHostEl ? proxyHostEl.value.trim() : undefined,
              port: proxyPortEl ? parseInt(proxyPortEl.value, 10) || 1080 : undefined,
              username: proxyUserEl ? proxyUserEl.value.trim() : undefined,
              password: proxyPassEl && proxyPassEl.value ? proxyPassEl.value : undefined,
            },
            vpn: {
              enabled: vpnEnabledEl ? vpnEnabledEl.checked : undefined,
            },
          },
        });

        TdApi.showToast('网络设置已保存（Headless 写入 network_settings.json）');

      } catch (e) {

        TdApi.showToast(String(e.message || e), 'err');

      }

    });

  }



  async function loadTransport() {

    try {

      var info = await TdApi.apiJson('/api/v1/transport');

      transportEl.textContent = info.active_mode + '（默认 ' + info.default_mode + '）';

      modesEl.innerHTML = '';

      (info.available_modes || []).forEach(function (mode) {

        var btn = document.createElement('button');

        btn.type = 'button';

        btn.className = 'btn-secondary btn-sm' + (mode === info.active_mode ? ' active-mode' : '');

        btn.textContent = mode === info.active_mode ? mode + ' · 当前' : '切换到 ' + mode;

        btn.disabled = mode === info.active_mode;

        btn.addEventListener('click', function () {

          switchMode(mode);

        });

        modesEl.appendChild(btn);

      });

      if (!(info.available_modes || []).length) {

        modesEl.innerHTML = '<p class="muted">未配置 Bot 或 User 模式，请检查 .env</p>';

      }

    } catch (e) {

      transportEl.textContent = '加载失败';

      TdApi.showToast(String(e.message || e), 'err');

    }

  }



  async function switchMode(mode) {
    var msg =
      mode === 'user'
        ? '切换为 User 模式后需在 Telegram 登录页完成用户绑定。确认切换？'
        : '切换为 Bot 模式后无需 User 登录，但需配置 TG_BOT_TOKEN。确认切换？';
    if (!confirm(msg)) return;

    try {

      await TdApi.apiJson('/api/v1/transport/mode', {

        method: 'POST',

        body: { mode: mode },

      });

      TdApi.showToast(
        '已切换为 ' +
          mode +
          '。文件索引已重置；User 模式请在本页点击「重建文件索引」或到文件列表刷新。',
      );

      if (mode === 'user') {
        var next = encodeURIComponent('/settings.html');
        location.href = '/telegram.html?next=' + next;
        return;
      }

      loadTransport();

      loadHealth();

    } catch (e) {

      TdApi.showToast(String(e.message || e), 'err');

    }

  }



  async function loadHealth() {
    try {
      var hv = await TdApi.fetchHealth();
      healthEl.textContent =
        'API v' +
        hv.version +
        ' · transport=' +
        hv.transport_mode +
        ' · ready=' +
        hv.ready +
        (hv.multi_tenant_enabled ? ' · 多租户' : '');
    } catch (e) {
      healthEl.textContent = String(e.message || e);
      TdApi.showToast('健康检查失败: ' + String(e.message || e), 'err');
    }
  }



  async function checkMetrics() {

    try {

      var res = await fetch('/metrics');

      if (res.status === 404 || res.status === 403) {

        metricsHint.textContent = 'Metrics 未启用（.env METRICS_ENABLED=false）';

        metricsLink.style.display = 'none';

        return;

      }

      if (res.ok) {

        metricsHint.textContent = 'Prometheus 文本格式 · 新标签页打开';

        metricsLink.href = '/metrics';

        metricsLink.style.display = 'inline-flex';

      } else {

        metricsHint.textContent = 'Metrics 探测失败: HTTP ' + res.status;

        metricsLink.style.display = 'none';

      }

    } catch (e) {

      metricsHint.textContent = '无法探测 /metrics';

    }

  }



  loadSettingsPanel();

  loadNetworkPanel();

  loadTransport();

  loadHealth();

  checkMetrics();

})();

