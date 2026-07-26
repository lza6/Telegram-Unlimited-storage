/* Drag-and-drop upload enhancement — adds full-page drop zone and paste upload.
 * Requires: TdApi (api-client.js), TdToast (notifications.js)
 * Usage: <script src="/assets/upload-drop.js"></script>
 */
(function (global) {
  'use strict';

  var dragCounter = 0;
  var overlay = null;

  function createOverlay() {
    if (overlay) return overlay;
    overlay = document.createElement('div');
    overlay.id = 'td-drop-overlay';
    overlay.setAttribute('aria-hidden', 'true');
    overlay.style.cssText =
      'position:fixed;inset:0;z-index:9998;background:rgba(37,99,235,0.12);border:3px dashed #2563eb;display:none;align-items:center;justify-content:center;pointer-events:none;';
    overlay.innerHTML =
      '<div style="background:#fff;border-radius:16px;padding:32px 48px;text-align:center;box-shadow:0 8px 32px rgba(0,0,0,0.15);">' +
      '<div style="font-size:48px;margin-bottom:12px;">📁</div>' +
      '<div style="font-size:20px;font-weight:700;color:#1e293b;">释放以开始上传</div>' +
      '<div style="font-size:14px;color:#64748b;margin-top:4px;">支持任意文件格式，自动分片上传</div>' +
      '</div>';
    document.body.appendChild(overlay);
    return overlay;
  }

  function handleDragEnter(e) {
    e.preventDefault();
    e.stopPropagation();
    dragCounter++;
    var ov = createOverlay();
    ov.style.display = 'flex';
  }

  function handleDragLeave(e) {
    e.preventDefault();
    e.stopPropagation();
    dragCounter--;
    if (dragCounter <= 0 && overlay) {
      overlay.style.display = 'none';
      dragCounter = 0;
    }
  }

  function handleDrop(e) {
    e.preventDefault();
    e.stopPropagation();
    dragCounter = 0;
    if (overlay) overlay.style.display = 'none';

    var files = e.dataTransfer && e.dataTransfer.files;
    if (!files || files.length === 0) return;

    // Try to set the file input so the existing upload pipeline picks them up
    var fileInput = document.getElementById('file-input');
    if (fileInput) {
      var dt = new DataTransfer();
      for (var i = 0; i < files.length; i++) {
        dt.items.add(files[i]);
      }
      fileInput.files = dt.files;
      fileInput.dispatchEvent(new Event('change', { bubbles: true }));
      if (typeof TdToast !== 'undefined') {
        TdToast.info('已选择 ' + files.length + ' 个文件，点击"开始上传"');
      }
    }
  }

  function handlePaste(e) {
    var items = e.clipboardData && e.clipboardData.items;
    if (!items) return;
    var imageFiles = [];
    for (var i = 0; i < items.length; i++) {
      var item = items[i];
      if (item.kind === 'file' && item.type.match(/^image\//)) {
        imageFiles.push(item.getAsFile());
      }
    }
    if (imageFiles.length === 0) return;

    var fileInput = document.getElementById('file-input');
    if (fileInput) {
      var dt = new DataTransfer();
      for (var j = 0; j < imageFiles.length; j++) {
        dt.items.add(imageFiles[j]);
      }
      fileInput.files = dt.files;
      fileInput.dispatchEvent(new Event('change', { bubbles: true }));
      if (typeof TdToast !== 'undefined') {
        TdToast.info('已粘贴 ' + imageFiles.length + ' 张图片，点击"开始上传"');
      }
    }
  }

  // Setup global drag-drop listeners
  document.addEventListener('dragenter', handleDragEnter);
  document.addEventListener('dragleave', handleDragLeave);
  document.addEventListener('dragover', function (e) {
    e.preventDefault();
    e.stopPropagation();
  });
  document.addEventListener('drop', handleDrop);
  document.addEventListener('paste', handlePaste);

  global.TdUploadDrop = { enabled: true };
})(window);