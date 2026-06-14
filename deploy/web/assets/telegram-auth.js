(function () {
  if (typeof TdApi === 'undefined' || !TdApi.requireLogin()) {
    return;
  }
  TdApi.initSidebar('telegram');

  function safeNext(raw) {
    if (!raw || typeof raw !== 'string') return '/dashboard.html';
    try {
      var u = new URL(raw, location.origin);
      if (u.origin !== location.origin) return '/dashboard.html';
      var path = u.pathname;
      if (!path.startsWith('/') || path.startsWith('//')) return '/dashboard.html';
      if (path.includes('login.html')) return '/dashboard.html';
      return path + u.search + u.hash;
    } catch (e) {
      return '/dashboard.html';
    }
  }

  function afterAuthSuccess() {
    location.href = safeNext(new URLSearchParams(location.search).get('next'));
  }

  async function authFetch(url, options) {
    return TdApi.apiFetch(url, options);
  }

  const msg = document.getElementById('msg');
  const adminAlert = document.getElementById('admin-alert');
  const phoneFieldset = document.getElementById('phone-fieldset');
  const dialSelect = document.getElementById('dial-code');
  const localInput = document.getElementById('local-phone');
  const btnSendCode = document.getElementById('btn-send-code');
  const qrStart = document.getElementById('qr-start');
  const tabPhone = document.getElementById('tab-phone');
  const tabQr = document.getElementById('tab-qr');
  const panelPhone = document.getElementById('panel-phone');
  const panelQr = document.getElementById('panel-qr');

  let userLoginAvailable = false;
  let qrPollTimer = null;

  async function readResponse(res) {
    const text = await res.text();
    if (!text) return { data: null, text: '' };
    try {
      return { data: JSON.parse(text), text };
    } catch {
      return { data: null, text };
    }
  }

  function showError(res, parsed) {
    const err =
      (parsed.data && (parsed.data.error || parsed.data.message)) || parsed.text;
    msg.textContent = err || '请求失败 (' + res.status + ')';
    msg.style.color = 'var(--err)';
  }

  function buildPhone() {
    const localRaw = (localInput.value || '').trim();
    if (localRaw.startsWith('+')) {
      return localRaw.replace(/[\s\-()]/g, '');
    }
    const dial = dialSelect.value;
    if (dial === 'custom') {
      if (localRaw.startsWith('+')) return localRaw.replace(/[\s\-()]/g, '');
      throw new Error('选择「其他」时，请在手机号框填写完整国际号码，例如 +441234567890');
    }
    let local = localRaw.replace(/\D/g, '');
    if (local.startsWith('0')) local = local.slice(1);
    if (dial === '86' && local.startsWith('86') && local.length > 11) {
      local = local.slice(2);
    }
    if (!local) throw new Error('请输入手机号');
    return '+' + dial + local;
  }

  function setUserLoginState(ok, hint, transportMode) {
    userLoginAvailable = ok;
    tabPhone.style.display = ok ? '' : 'none';
    tabQr.style.display = ok ? '' : 'none';
    if (!ok) {
      panelPhone.classList.add('hidden');
      panelQr.classList.add('hidden');
      adminAlert.classList.remove('hidden');
      if (transportMode === 'bot') {
        adminAlert.innerHTML =
          '<strong>当前为机器人模式</strong><br>' +
          'Bot 模式无需绑定 Telegram 用户账号；上传/下载走 <code>TG_BOT_TOKEN</code>。<br>' +
          '若需 User 模式（更大单文件/更高吞吐），在 <code>.env</code> 填写 <code>TELEGRAM_API_ID/HASH</code> 并在管理台切换传输模式。';
      } else {
        adminAlert.innerHTML =
          '<strong>Telegram API 尚未配置</strong><br>' +
          (hint ||
            '需要真实的 TELEGRAM_API_ID / TELEGRAM_API_HASH。') +
          '<br>编辑 <code>.env</code> → 重启服务 → 刷新本页。';
      }
      phoneFieldset.disabled = true;
      qrStart.disabled = true;
      return;
    }
    adminAlert.classList.add('hidden');
    phoneFieldset.disabled = false;
    qrStart.disabled = false;
    tabPhone.classList.add('active');
    tabQr.classList.remove('active');
    panelPhone.classList.remove('hidden');
    panelQr.classList.add('hidden');
  }

  function updatePhoneHint() {
    const hint = document.getElementById('phone-hint');
    if (dialSelect.value === 'custom') {
      hint.innerHTML =
        '请填写完整国际号码，例如 <strong>+8613800138000</strong>';
      localInput.placeholder = '+8613800138000';
    } else if (dialSelect.value === '86') {
      hint.innerHTML =
        '示例：选「中国 +86」后填 <strong>13800138000</strong>';
      localInput.placeholder = '13800138000';
    } else {
      hint.innerHTML = '示例：+' + dialSelect.value + ' 后接本地号码';
      localInput.placeholder = '本地号码';
    }
  }

  function renderQr(url) {
    const box = document.getElementById('qr-box');
    box.innerHTML = '';
    box.classList.remove('hidden');
    if (typeof QRCode !== 'undefined') {
      new QRCode(box, { text: url, width: 220, height: 220 });
    } else {
      box.textContent = url;
    }
    document.getElementById('qr-poll').classList.remove('hidden');
  }

  function stopQrPoll() {
    if (qrPollTimer) {
      clearInterval(qrPollTimer);
      qrPollTimer = null;
    }
  }

  async function pollQrOnce() {
    const res = await authFetch('/api/v1/auth/qr/poll');
    const parsed = await readResponse(res);
    const data = parsed.data || {};
    if (data.connected) {
      stopQrPoll();
      afterAuthSuccess();
      return true;
    }
    if (!res.ok) {
      showError(res, parsed);
      stopQrPoll();
    }
    return false;
  }

  function resetToPhoneStep() {
    document.getElementById('phone-form').classList.remove('hidden');
    document.getElementById('code-form').classList.add('hidden');
    document.getElementById('password-form').classList.add('hidden');
    document.getElementById('code-form').reset();
    msg.textContent = '验证码错误或会话已过期，请重新发送验证码';
    msg.style.color = 'var(--err)';
  }

  dialSelect.addEventListener('change', updatePhoneHint);
  updatePhoneHint();

  TdApi.fetchAuthStatus()
    .then(function (s) {
      if (s.transport_mode === 'bot' && s.connected) {
        msg.textContent = '机器人模式已就绪：' + (s.user || '');
        msg.style.color = '';
        setUserLoginState(false, s.hint, 'bot');
        adminAlert.innerHTML +=
          '<p style="margin-top:12px"><a href="/dashboard.html">返回控制台</a> · ' +
          '<a href="/settings.html">传输模式设置</a></p>';
        return;
      }
      if (s.connected && s.user && s.transport_mode === 'user') {
        msg.textContent = '已登录：' + s.user + '，正在跳转…';
        setTimeout(afterAuthSuccess, 800);
        return;
      }
      setUserLoginState(s.user_configured === true, s.hint, s.transport_mode);
      if (s.user_configured) {
        msg.textContent = '请绑定 Telegram 用户账号（User 模式）';
      }
    })
    .catch(function () {
      msg.textContent = '无法读取服务状态，请检查 API 是否在运行';
      msg.style.color = 'var(--err)';
    });

  tabPhone.onclick = function () {
    tabPhone.classList.add('active');
    tabQr.classList.remove('active');
    panelPhone.classList.remove('hidden');
    panelQr.classList.add('hidden');
    stopQrPoll();
  };
  tabQr.onclick = function () {
    tabQr.classList.add('active');
    tabPhone.classList.remove('active');
    panelQr.classList.remove('hidden');
    panelPhone.classList.add('hidden');
  };

  document.getElementById('phone-form').onsubmit = async function (e) {
    e.preventDefault();
    if (!userLoginAvailable) return;
    var phone;
    try {
      phone = buildPhone();
    } catch (err) {
      msg.textContent = err.message;
      msg.style.color = 'var(--err)';
      return;
    }
    btnSendCode.disabled = true;
    msg.textContent = '正在向 ' + phone + ' 发送验证码…';
    msg.style.color = '';
    try {
      var res = await authFetch('/api/v1/auth/phone/request', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ phone: phone }),
      });
      var parsed = await readResponse(res);
      if (res.ok) {
        msg.textContent = '验证码已发送，请在 Telegram 查看';
        document.getElementById('phone-form').classList.add('hidden');
        document.getElementById('code-form').classList.remove('hidden');
      } else {
        showError(res, parsed);
      }
    } catch (err) {
      msg.textContent = String(err.message || err);
      msg.style.color = 'var(--err)';
    } finally {
      btnSendCode.disabled = false;
    }
  };

  document.getElementById('code-form').onsubmit = async function (e) {
    e.preventDefault();
    var code = new FormData(e.target).get('code');
    try {
      var res = await authFetch('/api/v1/auth/phone/sign-in', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ code: code }),
      });
      var parsed = await readResponse(res);
      var data = parsed.data || {};
      if (res.ok && data.connected) {
        afterAuthSuccess();
      } else if (data.next_step === 'password') {
        document.getElementById('code-form').classList.add('hidden');
        document.getElementById('password-form').classList.remove('hidden');
        msg.textContent = '请输入两步验证密码';
      } else {
        showError(res, parsed);
        var errText = (parsed.data && (parsed.data.error || parsed.data.message)) || parsed.text || '';
        if (String(errText).includes('phone/request') || res.status === 400) {
          resetToPhoneStep();
        }
      }
    } catch (err) {
      msg.textContent = String(err.message || err);
      msg.style.color = 'var(--err)';
    }
  };

  document.getElementById('password-form').onsubmit = async function (e) {
    e.preventDefault();
    var password = new FormData(e.target).get('password');
    try {
      var res = await authFetch('/api/v1/auth/phone/password', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ password: password }),
      });
      var parsed = await readResponse(res);
      if (res.ok) afterAuthSuccess();
      else showError(res, parsed);
    } catch (err) {
      msg.textContent = String(err.message || err);
      msg.style.color = 'var(--err)';
    }
  };

  qrStart.onclick = async function () {
    if (!userLoginAvailable) return;
    stopQrPoll();
    msg.textContent = '正在生成二维码…';
    try {
      var res = await authFetch('/api/v1/auth/qr/start', { method: 'POST' });
      var parsed = await readResponse(res);
      if (!res.ok) {
        showError(res, parsed);
        return;
      }
      var data = parsed.data || {};
      if (data.authorized) {
        afterAuthSuccess();
        return;
      }
      if (!data.url) {
        msg.textContent = '未返回二维码链接';
        return;
      }
      renderQr(data.url);
      msg.textContent = '请用 Telegram App 扫描二维码';
      qrPollTimer = setInterval(function () {
        pollQrOnce().catch(function (err) {
          msg.textContent = String(err.message || err);
          msg.style.color = 'var(--err)';
          stopQrPoll();
        });
      }, 2500);
      pollQrOnce().catch(function (err) {
        msg.textContent = String(err.message || err);
        msg.style.color = 'var(--err)';
        stopQrPoll();
      });
    } catch (err) {
      msg.textContent = String(err.message || err);
      msg.style.color = 'var(--err)';
    }
  };

  document.getElementById('qr-poll').onclick = function () {
    pollQrOnce().catch(function (err) {
      msg.textContent = String(err.message || err);
      msg.style.color = 'var(--err)';
      stopQrPoll();
    });
  };

  window.addEventListener('beforeunload', stopQrPoll);
})();
