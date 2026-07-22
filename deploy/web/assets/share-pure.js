/* Share UX helpers — keep in sync with app/src/lib/sharePure.ts */
(function (global) {
  function formatShareCreateErrorMessage(err) {
    var msg = err != null ? String(err) : '';
    if (!msg) return '创建分享失败';
    if (msg.indexOf('bot_file_map') >= 0 || msg.indexOf('Bot download') >= 0) {
      return '该文件尚未建立 Bot 下载映射，无法创建分享。请先在 Bot 模式下通过 Bot 上传，或在设置中重建/同步索引后再试。';
    }
    if (msg.indexOf('asset index') >= 0) {
      return '该文件不在资产索引中，无法创建分享。请先在设置中重建文件索引。';
    }
    if (msg.indexOf('Access denied') >= 0 || msg.indexOf('another tenant') >= 0) {
      return '无权为该文件创建分享（租户隔离）。';
    }
    return msg;
  }

  global.TdSharePure = {
    formatShareCreateErrorMessage: formatShareCreateErrorMessage,
  };
})(typeof window !== 'undefined' ? window : globalThis);
