/**
 * Telegram Drive File Card Component (TASK-P1-02, v8.0).
 *
 * Renders a single file as a card with an SVG thumbnail, name, size and
 * category badge. Clicking the card opens a preview lightbox (images/PDF/video).
 *
 * Usage:
 *   <td-file-card
 *     data-id="100"
 *     data-name="report.pdf"
 *     data-size="4096"
 *     data-mime="application/pdf"
 *     data-download-url="/api/v1/files/100/download">
 *   </td-file-card>
 *   <script src="/components/td-file-card.js"></script>
 *
 * Thumbnail source: GET /api/v1/files/{id}/thumb (SVG placeholder, v8).
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

  function formatSize(bytes) {
    var b = Number(bytes) || 0;
    if (b < 1024) return b + ' B';
    if (b < 1048576) return (b / 1024).toFixed(1) + ' KB';
    if (b < 1073741824) return (b / 1048576).toFixed(1) + ' MB';
    return (b / 1073741824).toFixed(1) + ' GB';
  }

  // Lazy singleton lightbox, shared across all file cards on a page.
  var lightbox = null;
  function getLightbox() {
    if (lightbox) return lightbox;
    lightbox = document.createElement('div');
    lightbox.setAttribute('class', 'td-lightbox');
    lightbox.setAttribute('role', 'dialog');
    lightbox.setAttribute('aria-modal', 'true');
    lightbox.setAttribute('aria-label', '文件预览');
    lightbox.hidden = true;
    lightbox.innerHTML =
      '<button class="td-lightbox__close" type="button" aria-label="关闭预览">×</button>' +
      '<div class="td-lightbox__content"></div>';
    function close() {
      lightbox.hidden = true;
      lightbox.querySelector('.td-lightbox__content').innerHTML = '';
    }
    lightbox.querySelector('.td-lightbox__close').addEventListener('click', close);
    lightbox.addEventListener('click', function (ev) {
      if (ev.target === lightbox) close();
    });
    document.addEventListener('keydown', function (ev) {
      if (ev.key === 'Escape' && !lightbox.hidden) close();
    });
    document.body.appendChild(lightbox);
    return lightbox;
  }

  function buildPreview(card) {
    var mime = card.getAttribute('data-mime') || '';
    var name = card.getAttribute('data-name') || '';
    var dlUrl = card.getAttribute('data-download-url') || '';
    var id = card.getAttribute('data-id') || '';
    var content = '';

    if (mime.indexOf('image/') === 0 && dlUrl) {
      content = '<img src="' + escapeHtml(dlUrl) + '" alt="' + escapeHtml(name) + '" loading="lazy">';
    } else if (mime.indexOf('video/') === 0 && dlUrl) {
      content = '<video src="' + escapeHtml(dlUrl) + '" controls preload="metadata"></video>';
    } else if (mime.indexOf('audio/') === 0 && dlUrl) {
      content = '<audio src="' + escapeHtml(dlUrl) + '" controls></audio>';
    } else if (mime === 'application/pdf' && dlUrl) {
      // pdf.js would go here; for now an embed + download fallback.
      content = '<embed src="' + escapeHtml(dlUrl) + '" type="application/pdf" width="100%" height="100%">';
    } else {
      content = '<p class="td-lightbox__noop">暂无预览，请<a href="' + escapeHtml(dlUrl) +
        '" download="' + escapeHtml(name) + '">下载</a>查看。</p>';
    }
    return content;
  }

  class TdFileCard extends HTMLElement {
    connectedCallback() {
      var id = this.getAttribute('data-id') || '';
      var name = this.getAttribute('data-name') || '未命名';
      var size = this.getAttribute('data-size') || '0';
      var mime = this.getAttribute('data-mime') || 'application/octet-stream';
      var cat = this.getAttribute('data-category') || 'other';
      var dlUrl = this.getAttribute('data-download-url') || '';
      var thumbUrl = '/api/v1/files/' + encodeURIComponent(id) + '/thumb';

      var a = document.createElement('a');
      a.setAttribute('class', 'td-file-card');
      a.setAttribute('href', dlUrl || '#');
      a.setAttribute('data-cat', cat);
      a.setAttribute('tabindex', '0');
      a.setAttribute('role', 'button');
      a.setAttribute('aria-label', '预览文件 ' + name);
      a.innerHTML =
        '<img class="td-file-card__thumb" src="' + escapeHtml(thumbUrl) +
        '" alt="" loading="lazy" width="200" height="200" />' +
        '<div class="td-file-card__meta">' +
        '<span class="td-file-card__name" title="' + escapeHtml(name) + '">' +
        escapeHtml(name) + '</span>' +
        '<span class="td-file-card__size">' + formatSize(size) + '</span>' +
        '<span class="td-file-card__badge td-badge--' + escapeHtml(cat) + '">' +
        escapeHtml(cat) + '</span>' +
        '</div>';

      // Click → open lightbox preview (prevent default download navigation).
      a.addEventListener('click', function (ev) {
        if (!dlUrl) return;
        ev.preventDefault();
        var lb = getLightbox();
        lb.querySelector('.td-lightbox__content').innerHTML = buildPreview(this);
        lb.hidden = false;
      }.bind(this));

      // Keyboard accessible.
      a.addEventListener('keydown', function (ev) {
        if (ev.key === 'Enter' || ev.key === ' ') {
          ev.preventDefault();
          a.click();
        }
      });

      this.innerHTML = '';
      this.appendChild(a);
    }
  }

  customElements.define('td-file-card', TdFileCard);
})();
