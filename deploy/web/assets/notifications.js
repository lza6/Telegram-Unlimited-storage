/* Unified toast notification system — replaces ad-hoc toast elements.
 * Usage: TdToast.success("Upload complete"); TdToast.error("Failed");
 * Types: success, error, warning, info, progress
 */
(function (global) {
  'use strict';

  var CONTAINER_ID = 'td-toast-container';
  var DURATION = 4000; // auto-dismiss ms for non-progress toasts
  var MAX_TOASTS = 5;

  function ensureContainer() {
    var el = document.getElementById(CONTAINER_ID);
    if (!el) {
      el = document.createElement('div');
      el.id = CONTAINER_ID;
      el.setAttribute('role', 'status');
      el.setAttribute('aria-live', 'polite');
      el.setAttribute('aria-label', 'Notifications');
      el.style.cssText =
        'position:fixed;bottom:24px;right:24px;z-index:9999;display:flex;flex-direction:column-reverse;gap:8px;max-width:400px;pointer-events:none;';
      document.body.appendChild(el);
    }
    return el;
  }

  function createToast(type, message, duration) {
    var container = ensureContainer();
    // Enforce max toast limit
    var children = container.querySelectorAll('.td-toast');
    while (children.length >= MAX_TOASTS) {
      var oldest = children[children.length - 1];
      if (oldest) {
        oldest.remove();
        children = container.querySelectorAll('.td-toast');
      }
    }

    var toast = document.createElement('div');
    toast.className = 'td-toast td-toast-' + type;
    toast.setAttribute('role', 'alert');
    // Colors by type
    var colors = {
      success: { bg: '#16a34a', icon: '✓' },
      error: { bg: '#dc2626', icon: '✕' },
      warning: { bg: '#f59e0b', icon: '⚠' },
      info: { bg: '#2563eb', icon: 'ℹ' },
      progress: { bg: '#7c3aed', icon: '⟳' },
    };
    var c = colors[type] || colors.info;
    toast.style.cssText =
      'display:flex;align-items:center;gap:10px;padding:12px 16px;border-radius:8px;background:' +
      c.bg +
      ';color:#fff;font-size:14px;line-height:1.4;box-shadow:0 4px 12px rgba(0,0,0,0.15);pointer-events:auto;animation:td-toast-in 0.25s ease-out;cursor:pointer;';
    toast.innerHTML =
      '<span style="font-weight:700;flex-shrink:0;width:20px;text-align:center;">' +
      c.icon +
      '</span><span style="flex:1;">' +
      escapeHtml(message) +
      '</span>';
    toast.title = 'Click to dismiss';

    toast.addEventListener('click', function () {
      toast.style.animation = 'td-toast-out 0.2s ease-in forwards';
      setTimeout(function () {
        if (toast.parentNode) toast.remove();
      }, 200);
    });

    container.appendChild(toast);

    if (type !== 'progress' && duration > 0) {
      setTimeout(function () {
        if (!toast.parentNode) return;
        toast.style.animation = 'td-toast-out 0.2s ease-in forwards';
        setTimeout(function () {
          if (toast.parentNode) toast.remove();
        }, 200);
      }, duration);
    }

    return toast;
  }

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  // Inject keyframes once
  function injectStyles() {
    if (document.getElementById('td-toast-styles')) return;
    var style = document.createElement('style');
    style.id = 'td-toast-styles';
    style.textContent =
      '@keyframes td-toast-in{from{opacity:0;transform:translateY(12px)}to{opacity:1;transform:translateY(0)}}@keyframes td-toast-out{from{opacity:1;transform:translateY(0)}to{opacity:0;transform:translateY(-8px)}}';
    document.head.appendChild(style);
  }

  injectStyles();

  global.TdToast = {
    success: function (msg) {
      return createToast('success', msg, DURATION);
    },
    error: function (msg) {
      return createToast('error', msg, DURATION * 1.5);
    },
    warning: function (msg) {
      return createToast('warning', msg, DURATION);
    },
    info: function (msg) {
      return createToast('info', msg, DURATION);
    },
    progress: function (msg) {
      return createToast('progress', msg, 0); // never auto-dismiss
    },
    dismiss: function (toast) {
      if (toast && toast.parentNode) {
        toast.style.animation = 'td-toast-out 0.2s ease-in forwards';
        setTimeout(function () {
          if (toast.parentNode) toast.remove();
        }, 200);
      }
    },
  };
})(window);