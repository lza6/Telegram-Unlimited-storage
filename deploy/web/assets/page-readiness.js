/* Shared service readiness UX for dashboard / upload pages */

(function (global) {

  function transportHint(st) {

    if (st.connected) return null;

    return (

      st.hint ||

      (st.transport_mode === 'bot'

        ? '请检查 .env 中 TG_BOT_TOKEN 与 TG_STORAGE_CHANNEL_ID'

        : '请先完成 Telegram User 登录')

    );

  }



  function applyTransportStatus(tgDot, tgStatus, st, hv) {

    if (!tgDot || !tgStatus) return;

    var pure = global.TdWebPure;

    var uploadReady =

      pure && typeof pure.isWebTransportReady === 'function'

        ? pure.isWebTransportReady(hv, st)

        : !!(st && st.connected && hv && hv.ready);

    var modeLabel = st.transport_mode === 'bot' ? '机器人模式' : '应用模式';

    tgDot.className = 'status-dot ' + (uploadReady ? 'ok' : 'bad');

    tgStatus.textContent = uploadReady

      ? modeLabel + ' 已就绪：' + (st.user || '')

      : modeLabel + ' 未就绪 — ' + (transportHint(st) || '传输未配置完成');

  }



  /**

   * @param {object} [opts]

   * @param {string} [opts.uploadBtnSelector]

   * @param {string} [opts.bannerPrefix]

   * @param {function} [opts.onStatus] - (authStatus, health) => void

   * @param {function} [opts.onError] - (err) => void

   */

  async function refreshUploadReadiness(opts) {

    opts = opts || {};

    var uploadBtn = document.querySelector(opts.uploadBtnSelector || '#upload-btn');

    var serviceBanner = document.getElementById('service-banner');

    var tgDot = document.getElementById('tg-dot');

    var tgStatus = document.getElementById('tg-status');

    var apiHealthEl = document.getElementById('api-health');



    var gotAuth = false;

    var st = null;

    var hv = null;



    try {

      st = await global.TdApi.fetchAuthStatus();

      gotAuth = true;



      hv = await global.TdApi.fetchHealth();

      applyTransportStatus(tgDot, tgStatus, st, hv);



      if (apiHealthEl) {

        apiHealthEl.textContent =

          'API v' +

          hv.version +

          ' · transport=' +

          hv.transport_mode +

          ' · ready=' +

          hv.ready +

          (hv.presigned_download_enabled ? ' · 预签名下载' : '') +

          (hv.multi_tenant_enabled ? ' · 多租户' : '');

      }



      if (typeof opts.onStatus === 'function') opts.onStatus(st, hv);



      await global.TdApi.ensureTransportReady();

      if (serviceBanner) {

        serviceBanner.hidden = true;

        serviceBanner.textContent = '';

      }

      if (uploadBtn) uploadBtn.disabled = false;

      var folderSelect = document.querySelector(opts.folderSelectSelector || '#upload-folder');

      if (folderSelect) folderSelect.disabled = false;

      return true;

    } catch (e) {

      var hint = String(e.message || e);

      if (!gotAuth) {

        if (tgDot && tgStatus) {

          tgDot.className = 'status-dot bad';

          tgStatus.textContent = '无法连接 API 服务 — 请确认 Docker/服务已启动';

        }

        if (apiHealthEl) apiHealthEl.textContent = hint;

        if (serviceBanner) {

          serviceBanner.hidden = false;

          serviceBanner.textContent = '无法连接 API 服务';

        }

      } else {

        applyTransportStatus(tgDot, tgStatus, st, hv);

        if (apiHealthEl && hv) {

          apiHealthEl.textContent =

            'API v' +

            hv.version +

            ' · transport=' +

            hv.transport_mode +

            ' · ready=' +

            hv.ready +

            ' · 上传暂不可用';

        }

        if (serviceBanner) {

          serviceBanner.hidden = false;

          serviceBanner.textContent = (opts.bannerPrefix || '上传暂不可用：') + hint;

        }

      }

      if (uploadBtn) uploadBtn.disabled = true;

      var folderSelect = document.querySelector(opts.folderSelectSelector || '#upload-folder');

      if (folderSelect) folderSelect.disabled = true;

      if (typeof opts.onError === 'function') opts.onError(e);

      return false;

    }

  }



  global.TdPageReadiness = { refreshUploadReadiness: refreshUploadReadiness };

})(window);
