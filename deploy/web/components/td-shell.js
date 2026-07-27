/**
 * Telegram Drive Shell Component (TASK-U-01).
 *
 * Web Component that renders the shared sidebar + main slot across all pages.
 * Eliminates the 9-page sidebar duplication: edit navigation here once.
 *
 * Usage:
 *   <td-shell current="dashboard">
 *     <header class="page-header">...</header>
 *     <!-- page content -->
 *   </td-shell>
 *   <script src="/components/td-shell.js"></script>
 */
(function () {
  'use strict';

  var NAV_ITEMS = [
    { href: '/dashboard.html', key: 'dashboard', label: '概览 & 上传' },
    { href: '/files.html', key: 'files', label: '文件列表' },
    { href: '/shares.html', key: 'shares', label: '分享管理' },
    { href: '/transfers.html', key: 'transfers', label: '传输中心' },
    { href: '/settings.html', key: 'settings', label: '服务设置' },
    { href: '/upload.html', key: 'upload', label: '上传页 (tg-disk)' },
    { href: '/docs.html', key: 'docs', label: 'API 文档' },
    { href: '/telegram.html', key: 'telegram', label: 'Telegram 登录' },
  ];

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  class TdShell extends HTMLElement {
    connectedCallback() {
      var current = this.getAttribute('current') || '';
      var navHtml = NAV_ITEMS.map(function (item) {
        var isActive = item.key === current;
        var attrs = isActive ? ' class="active" aria-current="page"' : '';
        return '<a href="' + item.href + '" data-nav="' + item.key + '"' + attrs + '>' +
               escapeHtml(item.label) + '</a>';
      }).join('');

      var sidebar = '' +
        '<aside class="sidebar">' +
          '<strong class="brand-lockup">Telegram Drive</strong>' +
          '<nav class="sidebar-nav" aria-label="主导航">' + navHtml +
            '<button type="button" id="logout-btn" class="btn-secondary sidebar-logout">退出登录</button>' +
            '<button type="button" id="theme-toggle" class="theme-toggle" aria-label="切换主题">' +
              '<span class="theme-toggle-icon" aria-hidden="true">🌙</span>' +
              '<span class="theme-toggle-label">暗色模式</span>' +
            '</button>' +
          '</nav>' +
          '<p class="sidebar-note">高级配置与 API 说明可在设置和文档中完成。</p>' +
        '</aside>';

      this.innerHTML = '' +
        '<div class="layout">' +
          sidebar +
          '<main id="main-content" class="content" tabindex="-1">' +
            '<slot></slot>' +
          '</main>' +
        '</div>';

      // Wire up logout + theme toggle if TdApi helpers exist
      var logoutBtn = this.querySelector('#logout-btn');
      if (logoutBtn) {
        logoutBtn.addEventListener('click', function () {
          if (typeof TdApi !== 'undefined' && TdApi.logout) {
            TdApi.logout();
          } else {
            sessionStorage.clear();
            location.href = '/login.html';
          }
        });
      }
      var themeBtn = this.querySelector('#theme-toggle');
      if (themeBtn && typeof TdTheme !== 'undefined' && TdTheme.bindToggle) {
        TdTheme.bindToggle(themeBtn);
      }
    }
  }

  // Hide undefined custom element to prevent FOUC
  var style = document.createElement('style');
  style.textContent = 'td-shell:not(:defined){visibility:hidden;}';
  document.head.appendChild(style);

  customElements.define('td-shell', TdShell);
})();
