/* Shared REST client for Web admin — uses session ACCESS_PWD as X-Access-Pwd */
(function (global) {
  var toastTimer = null;
  function getAccessPwd() {
    return sessionStorage.getItem('td_access_pwd') || sessionStorage.getItem('pwd') || '';
  }

  function requireLogin(loginUrl) {
    if (!getAccessPwd()) {
      var next = encodeURIComponent(location.pathname + location.search);
      location.href = (loginUrl || '/login.html') + '?next=' + next;
      return false;
    }
    return true;
  }

  function initSidebar(activeNav) {
    document.querySelectorAll('[data-nav]').forEach(function (el) {
      var isActive = el.getAttribute('data-nav') === activeNav;
      el.classList.toggle('active', isActive);
      if (isActive) el.setAttribute('aria-current', 'page');
      else el.removeAttribute('aria-current');
    });
    var logout = document.getElementById('logout-btn');
    if (logout && !logout.dataset.bound) {
      logout.dataset.bound = '1';
      logout.addEventListener('click', function () {
        sessionStorage.removeItem('td_access_pwd');
        sessionStorage.removeItem('pwd');
        location.href = '/login.html';
      });
    }
  }

  function showToast(msg, type) {
    var el = document.getElementById('toast');
    if (!el) return;
    el.textContent = msg;
    var cls = 'toast';
    if (type === 'err') cls += ' toast-err';
    else if (type === 'info') cls += ' toast-info';
    el.className = cls;
    el.setAttribute('role', type === 'err' ? 'alert' : 'status');
    el.setAttribute('aria-live', type === 'err' ? 'assertive' : 'polite');
    el.hidden = false;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(function () {
      el.hidden = true;
      toastTimer = null;
    }, 4500);
  }

  async function apiFetch(path, options) {
    options = options || {};
    var pwd = getAccessPwd();
    if (!pwd) {
      requireLogin();
      throw new Error('Not logged in');
    }
    var headers = Object.assign({}, options.headers || {}, {
      'X-Access-Pwd': pwd,
      Accept: 'application/json',
    });
    var body = options.body;
    if (body && typeof body === 'object' && !(body instanceof FormData)) {
      headers['Content-Type'] = 'application/json';
      body = JSON.stringify(body);
    }
    var res = await fetch(path, Object.assign({}, options, { headers: headers, body: body }));
    if (res.status === 401) {
      sessionStorage.removeItem('td_access_pwd');
      sessionStorage.removeItem('pwd');
      requireLogin();
      throw new Error('Unauthorized');
    }
    return res;
  }

  async function apiJson(path, options) {
    var res = await apiFetch(path, options);
    var text = await res.text();
    var data = null;
    try {
      data = text ? JSON.parse(text) : null;
    } catch (e) {
      data = text;
    }
    if (!res.ok) {
      var msg =
        (data && data.error && data.error.message) ||
        (data && data.error) ||
        text ||
        res.statusText;
      throw new Error(typeof msg === 'string' ? msg : JSON.stringify(msg));
    }
    return data;
  }

  async function fetchHealth() {
    var res = await fetch('/api/v1/health', { headers: { Accept: 'application/json' } });
    var text = await res.text();
    var data = null;
    try {
      data = text ? JSON.parse(text) : null;
    } catch (e) {
      data = null;
    }
    if (!res.ok) {
      throw new Error((data && data.error) || text || res.statusText || 'Health check failed');
    }
    return data || {};
  }

  async function fetchAuthStatus() {
    var res = await fetch('/api/v1/auth/status', { headers: { Accept: 'application/json' } });
    var text = await res.text();
    var data = null;
    try {
      data = text ? JSON.parse(text) : null;
    } catch (e) {
      data = null;
    }
    if (!res.ok) {
      throw new Error((data && data.error) || text || res.statusText || 'Auth status failed');
    }
    return data || {};
  }

  async function ensureServiceReady() {
    var health = await fetchHealth();
    if (!health.ready) {
      var hint =
        health.transport_mode === 'bot'
          ? 'Bot 模式未配置完成，请先在设置中配置 Bot Token'
          : 'Telegram 会话未就绪，请先完成登录';
      throw new Error(hint);
    }
    if (health.transport_mode === 'user') {
      var st = await fetchAuthStatus();
      if (!st.connected) {
        throw new Error('Telegram 用户未登录，请先在 Telegram 登录页完成绑定');
      }
    }
    return health;
  }

  /** API process reachable — enough for DB-only mutations (share CRUD, Bot bulk delete) */
  async function ensureApiAvailable() {
    var health = await fetchHealth();
    if (!health || typeof health.version !== 'string') {
      throw new Error('API 服务不可用，请确认服务已启动');
    }
    return health;
  }

  async function ensureTransportReady() {
    return ensureServiceReady();
  }

  function formatSize(bytes) {
    var n = Number(bytes) || 0;
    if (n < 1024) return n + ' B';
    if (n < 1024 * 1024) return (n / 1024).toFixed(2) + ' KB';
    if (n < 1024 * 1024 * 1024) return (n / (1024 * 1024)).toFixed(2) + ' MB';
    return (n / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
  }

  async function copyToClipboard(text, okMsg) {
    try {
      await navigator.clipboard.writeText(String(text));
      showToast(okMsg || '已复制');
    } catch (e) {
      showToast('复制失败，请手动复制：' + String(text), 'err');
    }
  }

  global.TdApi = {
    getAccessPwd: getAccessPwd,
    requireLogin: requireLogin,
    initSidebar: initSidebar,
    showToast: showToast,
    apiFetch: apiFetch,
    apiJson: apiJson,
    fetchHealth: fetchHealth,
    fetchAuthStatus: fetchAuthStatus,
    ensureServiceReady: ensureServiceReady,
    ensureTransportReady: ensureTransportReady,
    ensureApiAvailable: ensureApiAvailable,
    formatSize: formatSize,
    copyToClipboard: copyToClipboard,
  };
})(window);
