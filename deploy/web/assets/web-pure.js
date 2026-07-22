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
    isWebApiReachable: function (health) {
      return !!health && typeof health.version === 'string';
    },
    isWebTransportReady: function (health, auth) {
      if (!health || !health.ready) return false;
      if ((health.transport_mode || '').toLowerCase() === 'user') {
        return !!(auth && auth.connected);
      }
      return true;
    },
    isWebDbMutationReady: function (health) {
      return global.TdWebPure.isWebApiReachable(health);
    },
    bulkDeleteRequiresTransport: function (transportMode) {
      return (transportMode || '').toLowerCase() === 'user';
    },
    SHARES_INVALIDATE_STORAGE_KEY: 'td-shares-invalidate',
    bumpSharesInvalidateStorage: function () {
      try {
        localStorage.setItem(
          global.TdWebPure.SHARES_INVALIDATE_STORAGE_KEY,
          String(Date.now()),
        );
      } catch (e) {
        /* private mode / quota */
      }
    },
    formatBulkDeleteConfirmMessage: function (count, transportMode) {
      var mode = (transportMode || 'bot').toLowerCase();
      var shareNote = '相关分享链接将一并撤销。';
      if (mode === 'user') {
        return (
          '确定删除选中的 ' +
          count +
          ' 个文件？User 模式下将同时删除 Telegram 消息，' +
          shareNote
        );
      }
      return (
        '确定删除选中的 ' +
        count +
        ' 条索引？Bot 模式下 Telegram 消息不会被删除，' +
        shareNote
      );
    },
    formatSingleDeleteConfirmMessage: function (transportMode) {
      var mode = (transportMode || 'user').toLowerCase();
      var shareNote = '相关分享链接将一并撤销。';
      if (mode === 'bot') {
        return '确定删除此文件索引？Telegram 消息不会被删除，' + shareNote;
      }
      return '确定删除此文件？将同时删除 Telegram 消息，' + shareNote;
    },
    formatDeleteSuccessToast: function (count, sharesRevoked) {
      if (count <= 0) return '没有可删除的条目';
      var sharePart;
      if (sharesRevoked != null) {
        sharePart =
          sharesRevoked > 0 ? '，已撤销 ' + sharesRevoked + ' 条分享链接' : '';
      } else {
        sharePart = '，相关分享链接已一并撤销';
      }
      if (count === 1) return '已删除 1 条' + sharePart;
      return '已删除 ' + count + ' 条' + sharePart;
    },
  };
})(typeof window !== 'undefined' ? window : globalThis);
