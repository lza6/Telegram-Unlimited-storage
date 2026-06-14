(function () {
  function safeNext(raw) {
    if (typeof TdWebPure !== 'undefined' && TdWebPure.safeNext) {
      return TdWebPure.safeNext(raw);
    }
    if (!raw || typeof raw !== 'string') return '/dashboard.html';
    try {
      var u = new URL(raw, location.origin);
      if (u.origin !== location.origin) return '/dashboard.html';
      var path = u.pathname;
      if (!path.startsWith('/') || path.startsWith('//')) return '/dashboard.html';
      if (path.includes('login.html')) return '/dashboard.html';
      return path + u.search + u.hash;
    } catch {
      return '/dashboard.html';
    }
  }

  const form = document.getElementById('login-form');
  const pwdInput = document.getElementById('pwd-input');
  const toggleBtn = document.getElementById('toggle-pwd');
  const submitBtn = document.getElementById('submit-btn');
  const errEl = document.getElementById('err');
  const toastEl = document.getElementById('toast');

  function showToast(msg) {
    if (!toastEl) return;
    toastEl.textContent = msg;
    toastEl.hidden = false;
    setTimeout(() => { toastEl.hidden = true; }, 4000);
  }
  window.showToast = showToast;

  function showError(msg) {
    errEl.textContent = msg;
    errEl.classList.remove('hidden');
    showToast(msg);
    pwdInput.setAttribute('aria-invalid', 'true');
  }

  toggleBtn.addEventListener('click', () => {
    const show = pwdInput.type === 'password';
    pwdInput.type = show ? 'text' : 'password';
    toggleBtn.textContent = show ? '隐藏' : '显示';
    toggleBtn.setAttribute('aria-label', show ? '隐藏密码' : '显示密码');
    toggleBtn.setAttribute('aria-pressed', show ? 'true' : 'false');
  });

  function setLoading(loading) {
    submitBtn.disabled = loading;
    submitBtn.classList.toggle('is-loading', loading);
    pwdInput.disabled = loading;
  }

  async function submitLogin(pwd) {
    const value = String(pwd ?? '').trim();
    if (!value) {
      showError('请输入管理密码');
      return false;
    }
    const fd = new FormData();
    fd.append('pwd', value);
    const res = await fetch('/verify', { method: 'POST', body: fd });
    if (res.ok) {
      sessionStorage.setItem('td_access_pwd', value);
      sessionStorage.setItem('pwd', value);
      location.href = safeNext(new URLSearchParams(location.search).get('next'));
      return true;
    }
    if (res.status === 503 || res.status === 502) {
      showError('服务正在重启，请等几秒后重试（刚执行过 sync.bat 时常见）');
    } else {
      if (res.status === 429) {
        const text = await res.text();
        showError(text || '请求过于频繁，请稍后再试');
      } else {
        showError('密码错误');
      }
    }
    pwdInput.focus();
    return false;
  }

  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    errEl.classList.add('hidden');
    pwdInput.setAttribute('aria-invalid', 'false');
    const pwd = pwdInput.value.trim();
    setLoading(true);
    try {
      await submitLogin(pwd);
    } catch {
      showError('网络错误，请检查 Docker 服务是否在 http://localhost:1334 运行');
    } finally {
      setLoading(false);
    }
  });

  // Do not auto-login from ?pwd= — password in URL is a security risk (history/referrer leaks).

  (function tryAlreadyLoggedIn() {
    var pwd = sessionStorage.getItem('td_access_pwd') || sessionStorage.getItem('pwd');
    if (!pwd) return;
    fetch('/api/v1/health', { headers: { 'X-Access-Pwd': pwd } })
      .then(function (res) {
        if (res.ok) {
          location.replace(safeNext(new URLSearchParams(location.search).get('next')));
        }
      })
      .catch(function () {
        showError('无法连接 API，请确认服务已启动');
      });
  })();
})();
