/* Populate upload folder selector — keep behavior aligned with app/src/lib/uploadPure.ts */
(function (global) {
  function escapeHtml(s) {
    if (typeof TdWebPure !== 'undefined' && TdWebPure.escapeHtml) {
      return TdWebPure.escapeHtml(s);
    }
    return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;');
  }

  async function populateFolderSelect(selectEl) {
    if (!selectEl) return;
    selectEl.innerHTML = '<option value="">Saved Messages（默认）</option>';
    try {
      var folders = await TdApi.apiJson('/api/v1/folders');
      (folders || []).forEach(function (f) {
        var opt = document.createElement('option');
        opt.value = String(f.id);
        opt.textContent = f.name || ('Folder ' + f.id);
        selectEl.appendChild(opt);
      });
    } catch (e) {
      if (typeof TdApi !== 'undefined' && TdApi.showToast) {
        TdApi.showToast('无法加载文件夹列表：' + String(e.message || e), 'err');
      }
    }
  }

  global.TdUploadFolder = {
    populateFolderSelect: populateFolderSelect,
  };
})(typeof window !== 'undefined' ? window : globalThis);
