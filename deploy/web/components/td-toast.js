/**
 * Telegram Drive Toast Component (TASK-P1-02, v8.0).
 *
 * A self-contained notification host. Pages append <td-toast-host> once, then
 * dispatch `window` events: `td:toast` with {type, message} to show a toast.
 *
 * Usage:
 *   <td-toast-host></td-toast-host>
 *   <script src="/components/td-toast.js"></script>
 *   window.dispatchEvent(new CustomEvent('td:toast', {
 *     detail: { type: 'success', message: '上传成功' }
 *   }));
 *
 * Types: success | error | warning | info (default).
 */
(function () {
  'use strict';

  function escapeHtml(s) {
    return String(s == null ? '' : s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  var ICONS = {
    success: '✓',
    error: '✕',
    warning: '!',
    info: 'i'
  };

  class TdToastHost extends HTMLElement {
    connectedCallback() {
      this.setAttribute('class', 'td-toast-host');
      this.setAttribute('role', 'status');
      this.setAttribute('aria-live', 'polite');
      this.setAttribute('aria-atomic', 'true');

      // Bind once; re-binding would stack handlers across re-renders.
      if (this._bound) return;
      this._bound = true;

      var host = this;
      window.addEventListener('td:toast', function (ev) {
        var detail = ev.detail || {};
        host._show(detail.type || 'info', detail.message || '');
      });
    }

    _show(type, message) {
      var toast = document.createElement('div');
      toast.setAttribute('class', 'td-toast td-toast--' + type);
      toast.setAttribute('role', 'alert');
      toast.innerHTML =
        '<span class="td-toast__icon" aria-hidden="true">' +
        (ICONS[type] || ICONS.info) + '</span>' +
        '<span class="td-toast__msg">' + escapeHtml(message) + '</span>' +
        '<button class="td-toast__close" type="button" aria-label="关闭">×</button>';

      var host = this;
      function dismiss() {
        toast.classList.add('td-toast--leave');
        setTimeout(function () { if (toast.parentNode) toast.parentNode.removeChild(toast); }, 200);
      }
      toast.querySelector('.td-toast__close').addEventListener('click', dismiss);
      this.appendChild(toast);

      // Auto-dismiss after 4s (errors stay longer at 8s).
      var ttl = type === 'error' ? 8000 : 4000;
      setTimeout(dismiss, ttl);
    }
  }

  customElements.define('td-toast-host', TdToastHost);
})();
