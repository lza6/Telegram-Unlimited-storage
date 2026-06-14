/* Pure Web helpers — keep in sync with app/src/lib/webPure.ts */
(function (global) {
  function safeNext(raw) {
    if (!raw || typeof raw !== 'string') return '/dashboard.html';
    var trimmed = String(raw).trim();
    if (!trimmed.startsWith('/')) return '/dashboard.html';
    try {
      var u = new URL(trimmed, location.origin);
      if (u.origin !== location.origin) return '/dashboard.html';
      var path = u.pathname;
      if (!path.startsWith('/') || path.startsWith('//')) return '/dashboard.html';
      if (path.includes('login.html')) return '/dashboard.html';
      return path + u.search + u.hash;
    } catch (e) {
      return '/dashboard.html';
    }
  }

  function safeHttpUrl(url) {
    var raw = String(url).trim();
    if (!/^https?:\/\//i.test(raw)) return '#';
    try {
      var u = new URL(raw, location.origin);
      if (u.protocol === 'http:' || u.protocol === 'https:') {
        return u.href;
      }
    } catch (e) {
      /* invalid */
    }
    return '#';
  }

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  global.TdWebPure = {
    safeNext: safeNext,
    safeHttpUrl: safeHttpUrl,
    escapeHtml: escapeHtml,
    rebuildIndexShouldToast: function (trigger) {
      return trigger === 'manual';
    },
    formatRebuildIndexSuccessToast: function (filesIndexed, foldersScanned) {
      return '索引已重建：' + filesIndexed + ' 个文件 / ' + foldersScanned + ' 个文件夹';
    },
    rebuildIndexShouldSurfaceBackgroundFailure: function (trigger) {
      return trigger === 'refresh' || trigger === 'search';
    },
    formatRebuildIndexBackgroundFailureMessage: function (err) {
      var detail = err != null ? String(err) : '';
      if (detail) {
        return '后台索引重建失败（列表仍可用）：' + detail;
      }
      return '后台索引重建未完成，将使用实时文件扫描';
    },
    shouldShowBotOnboarding: function (transportMode, connected, dismissed) {
      return transportMode === 'bot' && !connected && !dismissed;
    },
    shouldShowUserOnboarding: function (transportMode, connected, dismissed) {
      return transportMode === 'user' && !connected && !dismissed;
    },
  };
})(typeof window !== 'undefined' ? window : globalThis);
