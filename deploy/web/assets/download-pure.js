/* Pure Web download helpers — keep in sync with app/src/lib/downloadPure.ts */

(function (global) {

  function shouldBlockDuplicateDownload(inFlightIds, fileId) {

    return inFlightIds.has(String(fileId));

  }



  function computeDownloadPercent(bytesRead, totalBytes) {

    if (totalBytes == null || totalBytes <= 0) return null;

    var pct = Math.floor((bytesRead / totalBytes) * 100);

    return Math.min(100, Math.max(0, pct));

  }



  function formatDownloadProgressLabel(percent) {

    if (percent == null) return '下载中…';

    return '下载中 ' + percent + '%';

  }



  function deriveWebDownloadButtonState(inFlight, percent) {

    if (inFlight) {

      return { label: formatDownloadProgressLabel(percent == null ? null : percent), inFlight: true };

    }

    return { label: '下载', inFlight: false };

  }



  function resolveBlobDownloadFilename(file) {

    if (!file) return 'download';

    return file.name || file.filename || 'download';

  }



  function buildDownloadStartToast(file) {

    return '正在下载「' + resolveBlobDownloadFilename(file) + '」…';

  }



  function parseContentLengthHeader(header) {

    if (!header) return null;

    var n = parseInt(header, 10);

    return isFinite(n) && n > 0 ? n : null;

  }



  async function consumeStreamWithProgress(reader, totalBytes, onProgress) {

    var chunks = [];

    var received = 0;

    while (true) {

      var result = await reader.read();

      if (result.done) break;

      if (result.value && result.value.length > 0) {

        chunks.push(result.value);

        received += result.value.length;

        if (onProgress) onProgress(computeDownloadPercent(received, totalBytes));

      }

    }

    if (onProgress) {

      var finalPct = computeDownloadPercent(received, totalBytes);

      onProgress(finalPct != null ? finalPct : 100);

    }

    return chunks;

  }



  async function readResponseBlobWithProgress(res, onProgress) {

    var total = parseContentLengthHeader(res.headers.get('Content-Length'));

    if (!res.body || total == null) {

      return res.blob();

    }

    var chunks = await consumeStreamWithProgress(res.body.getReader(), total, onProgress);

    var type = res.headers.get('Content-Type') || 'application/octet-stream';

    return new Blob(chunks, { type: type });

  }



  global.TdDownloadPure = {

    shouldBlockDuplicateDownload: shouldBlockDuplicateDownload,

    computeDownloadPercent: computeDownloadPercent,

    formatDownloadProgressLabel: formatDownloadProgressLabel,

    deriveWebDownloadButtonState: deriveWebDownloadButtonState,

    resolveBlobDownloadFilename: resolveBlobDownloadFilename,

    buildDownloadStartToast: buildDownloadStartToast,

    parseContentLengthHeader: parseContentLengthHeader,

    consumeStreamWithProgress: consumeStreamWithProgress,

    readResponseBlobWithProgress: readResponseBlobWithProgress,

  };

})(typeof window !== 'undefined' ? window : globalThis);

