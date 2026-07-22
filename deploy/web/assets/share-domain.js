/* Shared share-domain helpers — server ui_settings.json + client fallback */
(function (global) {
  var cachedDomain = null;

  function getLocalShareDomain() {
    return localStorage.getItem('td_share_domain') || '';
  }

  function setLocalShareDomain(value) {
    localStorage.setItem('td_share_domain', (value || '').trim());
    cachedDomain = (value || '').trim();
  }

  async function loadShareDomainFromServer() {
    if (typeof TdApi === 'undefined' || !TdApi.getAccessPwd()) {
      cachedDomain = getLocalShareDomain();
      return cachedDomain;
    }
    try {
      var data = await TdApi.apiJson('/api/v1/settings');
      var domain = (data.share_domain || '').trim();
      if (domain) {
        setLocalShareDomain(domain);
        return domain;
      }
      cachedDomain = getLocalShareDomain();
      return cachedDomain;
    } catch (e) {
      cachedDomain = getLocalShareDomain();
      if (typeof TdApi !== 'undefined' && TdApi.showToast) {
        TdApi.showToast('无法从服务端加载分享域名，使用本地缓存', 'info');
      }
      return cachedDomain;
    }
  }

  async function saveShareDomainToServer(domain) {
    var trimmed = (domain || '').trim();
    if (typeof TdApi === 'undefined' || !TdApi.getAccessPwd()) {
      setLocalShareDomain(trimmed);
      return trimmed;
    }
    await TdApi.apiJson('/api/v1/settings', {
      method: 'PUT',
      body: { share_domain: trimmed },
    });
    setLocalShareDomain(trimmed);
    try {
      var refreshed = await TdApi.apiJson('/api/v1/settings');
      return refreshed;
    } catch (e) {
      if (typeof TdApi !== 'undefined' && TdApi.showToast) {
        TdApi.showToast('分享域名已保存，但刷新生效状态失败', 'info');
      }
      return { share_domain: trimmed };
    }
  }

  function applyShareDomain(link, domainOverride) {
    var domain = (domainOverride != null ? domainOverride : cachedDomain || getLocalShareDomain()).trim();
    if (!domain || !link) return link;
    try {
      var url = new URL(link);
      if (domain.startsWith('http://') || domain.startsWith('https://')) {
        var base = new URL(domain);
        return base.origin + url.pathname + url.search + url.hash;
      }
      return url.protocol + '//' + domain.replace(/^\/+|\/+$/g, '') + url.pathname + url.search + url.hash;
    } catch (e) {
      return link;
    }
  }

  global.TdShareDomain = {
    getLocalShareDomain: getLocalShareDomain,
    setLocalShareDomain: setLocalShareDomain,
    loadShareDomainFromServer: loadShareDomainFromServer,
    saveShareDomainToServer: saveShareDomainToServer,
    applyShareDomain: applyShareDomain,
    getCached: function () {
      return cachedDomain || getLocalShareDomain();
    },
  };
})(window);
