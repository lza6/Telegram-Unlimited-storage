/* tg-disk compatible chunked upload — shared by dashboard.html & upload.html */
(function (global) {
  function getAccessPwd() {
    if (typeof TdApi !== 'undefined' && TdApi.getAccessPwd) {
      return TdApi.getAccessPwd();
    }
    return sessionStorage.getItem('td_access_pwd') || sessionStorage.getItem('pwd') || '';
  }

  function requireLogin(loginUrl) {
    if (typeof TdApi !== 'undefined' && TdApi.requireLogin) {
      return TdApi.requireLogin(loginUrl);
    }
    if (!getAccessPwd()) {
      location.href = loginUrl || '/login.html';
      return false;
    }
    return true;
  }

  function escapeHtml(s) {
    if (typeof TdWebPure !== 'undefined' && TdWebPure.escapeHtml) {
      return TdWebPure.escapeHtml(s);
    }
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  function safeHttpUrl(url) {
    if (typeof TdWebPure !== 'undefined' && TdWebPure.safeHttpUrl) {
      return TdWebPure.safeHttpUrl(url);
    }
    try {
      var u = new URL(String(url), location.origin);
      if (u.protocol === 'http:' || u.protocol === 'https:') {
        return u.href;
      }
    } catch (e) {
      /* invalid */
    }
    return '#';
  }

  function showUploadError(msg) {
    if (typeof TdApi !== 'undefined' && typeof TdApi.showToast === 'function') {
      TdApi.showToast(msg, 'err');
    } else {
      alert(msg);
    }
  }

  function formatSize(bytes) {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(2) + ' KB';
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(2) + ' MB';
    return (bytes / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
  }

  function newUploadSessionId() {
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
      return crypto.randomUUID();
    }
    return 'sess-' + Date.now().toString(36) + '-' + Math.random().toString(36).slice(2, 10);
  }

  function parseUploadFolderId(raw) {
    if (!raw || !String(raw).trim()) return null;
    var n = parseInt(String(raw).trim(), 10);
    return Number.isFinite(n) && n > 0 ? n : null;
  }

  function getUploadFolderId(options) {
    if (!options.folderSelectSelector) return null;
    var el = document.querySelector(options.folderSelectSelector);
    if (!el) return null;
    return parseUploadFolderId(el.value);
  }

  function appendFolderId(formData, folderId) {
    if (folderId != null) formData.append('folder_id', String(folderId));
  }

  async function fetchUploadProgressAuthQuery(sessionId, pwd) {
    var accessPwd = pwd || getAccessPwd();
    if (!accessPwd || !sessionId) return '';
    try {
      var res = await fetch('/upload_progress_token', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Access-Pwd': accessPwd,
        },
        body: JSON.stringify({ session_id: sessionId }),
      });
      if (res.ok) {
        var data = await res.json();
        if (data.token && data.expires_at) {
          return (
            '&exp=' +
            encodeURIComponent(String(data.expires_at)) +
            '&token=' +
            encodeURIComponent(data.token)
          );
        }
      }
    } catch (e) {
      console.warn('upload progress token failed', e);
    }
    return '&pwd=' + encodeURIComponent(accessPwd);
  }

  async function subscribeUploadProgress(sessionId, onProgress, pwd) {
    if (!sessionId) return null;
    var authQ = await fetchUploadProgressAuthQuery(sessionId, pwd);

    if (typeof EventSource !== 'undefined') {
      var url = '/upload_events?session_id=' + encodeURIComponent(sessionId) + authQ;
      var source = new EventSource(url);
      var pollTimer = null;

      source.onmessage = function (ev) {
        try {
          var data = JSON.parse(ev.data);
          if (typeof onProgress === 'function') onProgress(data);
          if (data.status === 'failed') {
            source.close();
            if (pollTimer) clearInterval(pollTimer);
          } else if (data.status === 'completed') {
            source.close();
            if (pollTimer) clearInterval(pollTimer);
          }
        } catch (e) {
          console.warn('upload progress parse failed', e);
        }
      };
      source.onerror = function () {
        source.close();
        if (!pollTimer && authQ && (authQ.indexOf('token=') >= 0 || authQ.indexOf('pwd=') >= 0)) {
          var pollFailCount = 0;
          pollTimer = setInterval(function () {
            fetch('/upload_status?session_id=' + encodeURIComponent(sessionId) + authQ)
              .then(function (r) { return r.ok ? r.json() : null; })
              .then(function (data) {
                if (!data) {
                  pollFailCount += 1;
                  if (pollFailCount >= 3 && typeof TdApi !== 'undefined' && TdApi.showToast) {
                    TdApi.showToast('无法获取上传进度，请查看文件行状态', 'err');
                    clearInterval(pollTimer);
                  }
                  return;
                }
                pollFailCount = 0;
                if (typeof onProgress === 'function') {
                  onProgress({
                    uploaded_chunks: data.uploaded_chunks,
                    total_chunks: data.total_chunks,
                    status: data.status,
                    message: data.message,
                  });
                }
                if (data.status === 'completed' || data.status === 'failed') {
                  clearInterval(pollTimer);
                }
              })
              .catch(function () {
                pollFailCount += 1;
                if (pollFailCount >= 3 && typeof TdApi !== 'undefined' && TdApi.showToast) {
                  TdApi.showToast('无法获取上传进度，请查看文件行状态', 'err');
                  clearInterval(pollTimer);
                }
              });
          }, 2000);
        }
      };
      return {
        close: function () {
          source.close();
          if (pollTimer) clearInterval(pollTimer);
        },
      };
    }

    if (typeof WebSocket !== 'undefined') {
      var proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
      var wsUrl = proto + '//' + location.host + '/upload_ws?session_id=' + encodeURIComponent(sessionId) + authQ;
      var ws = new WebSocket(wsUrl);
      var wsPollTimer = null;
      function beginWsStatusPoll() {
        if (wsPollTimer || !authQ || (authQ.indexOf('token=') < 0 && authQ.indexOf('pwd=') < 0)) return;
        var pollFailCount = 0;
        wsPollTimer = setInterval(function () {
          fetch('/upload_status?session_id=' + encodeURIComponent(sessionId) + authQ)
            .then(function (r) { return r.ok ? r.json() : null; })
            .then(function (data) {
              if (!data) {
                pollFailCount += 1;
                if (pollFailCount >= 3 && typeof TdApi !== 'undefined' && TdApi.showToast) {
                  TdApi.showToast('无法获取上传进度，请查看文件行状态', 'err');
                  clearInterval(wsPollTimer);
                }
                return;
              }
              pollFailCount = 0;
              if (typeof onProgress === 'function') {
                onProgress({
                  uploaded_chunks: data.uploaded_chunks,
                  total_chunks: data.total_chunks,
                  status: data.status,
                  message: data.message,
                });
              }
              if (data.status === 'completed' || data.status === 'failed') {
                clearInterval(wsPollTimer);
              }
            })
            .catch(function () {
              pollFailCount += 1;
              if (pollFailCount >= 3 && typeof TdApi !== 'undefined' && TdApi.showToast) {
                TdApi.showToast('无法获取上传进度，请查看文件行状态', 'err');
                clearInterval(wsPollTimer);
              }
            });
        }, 2000);
      }
      ws.onmessage = function (ev) {
        try {
          var data = JSON.parse(ev.data);
          if (typeof onProgress === 'function') onProgress(data);
          if (data.status === 'completed' || data.status === 'failed') {
            ws.close();
          }
        } catch (e) {
          console.warn('upload ws parse failed', e);
        }
      };
      ws.onerror = function () {
        ws.close();
        beginWsStatusPoll();
      };
      return {
        close: function () {
          ws.close();
          if (wsPollTimer) clearInterval(wsPollTimer);
        },
      };
    }

    return null;
  }

  async function fetchWithRetry(url, options, maxAttempts, on503Wait) {
    const attempts = maxAttempts || 8;
    for (let i = 0; i < attempts; i++) {
      const response = await fetch(url, options);
      if (response.status !== 503) return response;
      const retryAfter = parseInt(response.headers.get('Retry-After') || '2', 10);
      const waitMs = Math.min(30000, retryAfter * 1000 * (i + 1));
      if (typeof on503Wait === 'function') {
        on503Wait({ attempt: i + 1, attempts, waitMs, retryAfter });
      }
      await new Promise((resolve) => setTimeout(resolve, waitMs));
    }
    return fetch(url, options);
  }

  class UploadQueue {
    constructor(concurrency) {
      this.concurrency = concurrency;
      this.running = 0;
      this.queue = [];
    }
    async add(task) {
      while (this.running >= this.concurrency) {
        await new Promise((resolve) => this.queue.push(resolve));
      }
      this.running++;
      try {
        return await task();
      } finally {
        this.running--;
        const resolve = this.queue.shift();
        if (resolve) resolve();
      }
    }
  }

  function initChunkedUpload(options) {
    const dropZone = document.querySelector(options.dropSelector);
    const fileInput = document.querySelector(options.inputSelector);
    const fileList = document.querySelector(options.listSelector);
    const uploadBtn = document.querySelector(options.uploadBtnSelector);
    const statsEl = document.querySelector(options.statsSelector);
    const fileCountEl = document.querySelector(options.fileCountSelector);
    const totalSizeEl = document.querySelector(options.totalSizeSelector);
    const modal = document.querySelector(options.modalSelector);
    const resultLinks = document.querySelector(options.resultLinksSelector);
    const retryStatusEl = options.retryStatusSelector
      ? document.querySelector(options.retryStatusSelector)
      : null;

    function on503Wait(info) {
      const secs = Math.ceil(info.waitMs / 1000);
      const msg =
        '上传队列繁忙 (503)，' + secs + ' 秒后重试 (' + info.attempt + '/' + info.attempts + ')';
      if (retryStatusEl) {
        retryStatusEl.textContent = msg;
        retryStatusEl.classList.remove('hidden');
      }
      showToast(msg);
    }

    let selectedFiles = [];
    let fileIdCounter = 0;
    let CHUNK_SIZE = 20 * 1024 * 1024;
    let CONCURRENT_UPLOADS = 4;
    let CONCURRENT_FILES = 3;

    if (typeof TdShareDomain !== 'undefined') {
      TdShareDomain.loadShareDomainFromServer();
    }

    (async function loadConfig() {
      try {
        const response = await fetch('/config');
        if (response.ok) {
          const config = await response.json();
          CHUNK_SIZE = config.chunk_size_mb * 1024 * 1024;
          CONCURRENT_UPLOADS = config.chunk_concurrent;
          CONCURRENT_FILES = config.files_concurrent;
        }
      } catch (e) {
        console.warn('config load failed', e);
        if (typeof TdApi !== 'undefined' && TdApi.showToast) {
          TdApi.showToast('无法加载上传配置，使用默认值', 'info');
        }
      }
    })();

    dropZone.addEventListener('click', () => fileInput.click());
    dropZone.addEventListener('dragover', (e) => {
      e.preventDefault();
      dropZone.classList.add('dragover');
    });
    dropZone.addEventListener('dragleave', () => dropZone.classList.remove('dragover'));
    dropZone.addEventListener('drop', (e) => {
      e.preventDefault();
      dropZone.classList.remove('dragover');
      handleFiles(e.dataTransfer.files);
    });
    fileInput.addEventListener('change', () => handleFiles(fileInput.files));

    function handleFiles(files) {
      const newFiles = Array.from(files).map((file) => ({
        id: fileIdCounter++,
        file,
        uploading: false,
        uploaded: false,
      }));
      selectedFiles = selectedFiles.concat(newFiles);
      if (statsEl) statsEl.style.display = 'flex';
      if (fileCountEl) fileCountEl.textContent = String(selectedFiles.length);
      if (totalSizeEl) {
        const totalBytes = selectedFiles.reduce((sum, item) => sum + item.file.size, 0);
        totalSizeEl.textContent = formatSize(totalBytes);
      }
      newFiles.forEach((fileItem) => {
        const div = document.createElement('div');
        div.className = 'file-item';
        div.id = 'file-item-' + fileItem.id;
        div.innerHTML =
          '<div class="file-info">' +
          '<div class="file-name">' + fileItem.file.name + '</div>' +
          '<div class="file-size">' + formatSize(fileItem.file.size) + '</div>' +
          '</div>' +
          '<div class="progress-bar"><div id="bar-' +
          fileItem.id +
          '" class="progress-bar-inner"></div></div>' +
          '<div id="status-' +
          fileItem.id +
          '" class="file-status">等待上传…</div>';
        fileList.appendChild(div);
      });
    }

    async function uploadSingleFile(file, fileId, pwd) {
      const totalChunks = Math.ceil(file.size / CHUNK_SIZE);
      const statusEl = document.getElementById('status-' + fileId);
      const progressBar = document.getElementById('bar-' + fileId);
      const folderId = getUploadFolderId(options);

      if (file.size <= CHUNK_SIZE) {
        statusEl.textContent = '上传中…';
        const formData = new FormData();
        formData.append('pwd', pwd);
        formData.append('file', file);
        appendFolderId(formData, folderId);
        const response = await fetchWithRetry('/upload', { method: 'POST', body: formData }, undefined, on503Wait);
        if (!response.ok) throw new Error(await response.text());
        progressBar.style.width = '100%';
        statusEl.textContent = '完成';
        return response.json();
      }

      statusEl.textContent = '分片上传 (共 ' + totalChunks + ' 片，并发 ' + CONCURRENT_UPLOADS + ')…';
      const sessionId = newUploadSessionId();
      const chunkIds = new Array(totalChunks);
      let uploadedChunks = 0;
      const queue = new UploadQueue(CONCURRENT_UPLOADS);
      const uploadTasks = [];
      let progressFailed = null;
      const progressSource = await subscribeUploadProgress(sessionId, function (ev) {
        if (ev.status === 'failed') {
          progressFailed = ev.message || '上传失败';
          statusEl.textContent = progressFailed;
          return;
        }
        if (ev.uploaded_chunks != null && ev.total_chunks > 0) {
          progressBar.style.width = (ev.uploaded_chunks / ev.total_chunks) * 100 + '%';
          statusEl.textContent = '上传分片 ' + ev.uploaded_chunks + '/' + ev.total_chunks + '…';
        }
      }, pwd);

      try {
        for (let chunkIndex = 0; chunkIndex < totalChunks; chunkIndex++) {
          const task = queue.add(async () => {
            const start = chunkIndex * CHUNK_SIZE;
            const end = Math.min(start + CHUNK_SIZE, file.size);
            const chunk = file.slice(start, end);
            const formData = new FormData();
            formData.append('pwd', pwd);
            formData.append('session_id', sessionId);
            formData.append('chunk', chunk);
            formData.append('chunk_index', String(chunkIndex));
            formData.append('total_chunks', String(totalChunks));
            formData.append('filename', file.name);
            const response = await fetchWithRetry('/upload_chunk', { method: 'POST', body: formData }, undefined, on503Wait);
            if (!response.ok) {
              throw new Error('分片 ' + (chunkIndex + 1) + ' 失败: ' + (await response.text()));
            }
            const data = await response.json();
            chunkIds[chunkIndex] = data.file_id;
            uploadedChunks++;
            progressBar.style.width = (uploadedChunks / totalChunks) * 100 + '%';
            statusEl.textContent = '上传分片 ' + uploadedChunks + '/' + totalChunks + '…';
          });
          uploadTasks.push(task);
        }
        await Promise.all(uploadTasks);
        if (progressFailed) throw new Error(progressFailed);

        statusEl.textContent = '合并分片…';
        const mergeFormData = new FormData();
        mergeFormData.append('pwd', pwd);
        mergeFormData.append('filename', file.name);
        mergeFormData.append('session_id', sessionId);
        mergeFormData.append('chunk_ids', JSON.stringify(chunkIds));
        appendFolderId(mergeFormData, folderId);
        const mergeResponse = await fetchWithRetry('/merge_chunks', { method: 'POST', body: mergeFormData }, undefined, on503Wait);
        if (!mergeResponse.ok) throw new Error('合并失败: ' + (await mergeResponse.text()));
        statusEl.textContent = '完成';
        return mergeResponse.json();
      } finally {
        if (progressSource && typeof progressSource.close === 'function') {
          progressSource.close();
        }
      }
    }

    function showResultModal(list, partial) {
      var titleEl = modal.querySelector('h2, h3, .modal-title');
      if (titleEl) {
        titleEl.textContent = partial ? '部分上传成功' : '上传成功';
      }
      resultLinks.innerHTML = '';
      list.forEach((file) => {
        var downloadUrl = file.download_url;
        if (typeof TdShareDomain !== 'undefined') {
          downloadUrl = TdShareDomain.applyShareDomain(downloadUrl);
        }
        var safeUrl = safeHttpUrl(downloadUrl);
        const html = '<a href="' + escapeHtml(safeUrl) + '" target="_blank" rel="noopener noreferrer">点击下载</a>';
        const md = '[点击下载](' + downloadUrl + ')';
        const bb = '[url=' + downloadUrl + ']点击下载[/url]';
        const div = document.createElement('div');
        div.className = 'result-item';
        div.innerHTML =
          '<div class="result-head">' +
          '<strong>' + escapeHtml(file.filename) + '</strong>' +
          '<a class="btn-sm" href="' + escapeHtml(safeUrl) + '" target="_blank" rel="noopener noreferrer">直接下载</a>' +
          '</div>' +
          '<label class="copy-block">URL <button type="button" class="copy-btn" data-copy="url">复制</button></label>' +
          '<textarea readonly class="copy-area" data-kind="url">' + escapeHtml(downloadUrl) + '</textarea>' +
          '<label class="copy-block">HTML <button type="button" class="copy-btn" data-copy="html">复制</button></label>' +
          '<textarea readonly class="copy-area" data-kind="html">' + escapeHtml(html) + '</textarea>' +
          '<div class="copy-grid">' +
          '<div><label class="copy-block">Markdown <button type="button" class="copy-btn">复制</button></label>' +
          '<textarea readonly class="copy-area">' + escapeHtml(md) + '</textarea></div>' +
          '<div><label class="copy-block">BBCode <button type="button" class="copy-btn">复制</button></label>' +
          '<textarea readonly class="copy-area">' + escapeHtml(bb) + '</textarea></div>' +
          '</div>';
        resultLinks.appendChild(div);
      });
      resultLinks.querySelectorAll('.copy-btn').forEach((btn) => {
        btn.addEventListener('click', async () => {
          const area = btn.closest('.copy-block')?.nextElementSibling || btn.parentElement?.nextElementSibling;
          if (area && area.tagName === 'TEXTAREA') {
            const text = area.value;
            if (typeof TdApi !== 'undefined' && TdApi.copyToClipboard) {
              try {
                await TdApi.copyToClipboard(text);
                btn.textContent = '已复制';
                setTimeout(() => { btn.textContent = '复制'; }, 1500);
                return;
              } catch (e) {
                TdApi.showToast(String(e.message || e), 'err');
                return;
              }
            }
            area.select();
            var ok = document.execCommand('copy');
            if (!ok && typeof TdApi !== 'undefined') {
              TdApi.showToast('复制失败，请手动选择文本', 'err');
              return;
            }
            btn.textContent = '已复制';
            setTimeout(() => { btn.textContent = '复制'; }, 1500);
          }
        });
      });
      modal.style.display = 'flex';
    }

    uploadBtn.addEventListener('click', async () => {
      const pwd = getAccessPwd();
      if (!pwd) {
        location.href = options.loginUrl || '/login.html';
        return;
      }
      if (typeof TdApi !== 'undefined' && TdApi.ensureServiceReady) {
        try {
          await TdApi.ensureServiceReady();
        } catch (e) {
          if (typeof TdApi.showToast === 'function') {
            TdApi.showToast(String(e.message || e), 'err');
          } else {
            showUploadError(String(e.message || e));
          }
          return;
        }
      }
      uploadBtn.disabled = true;
      uploadBtn.textContent = '上传中…';
      const uploadResponses = [];
      let uploadFailCount = 0;
      const pendingFiles = selectedFiles.filter(function (f) { return !f.uploaded && !f.uploading; });
      if (pendingFiles.length === 0) {
        uploadBtn.disabled = false;
        uploadBtn.textContent = options.uploadBtnLabel || '开始上传';
        if (typeof TdApi !== 'undefined' && TdApi.showToast) {
          TdApi.showToast('请先选择要上传的文件', 'err');
        } else {
          showUploadError('请先选择要上传的文件');
        }
        return;
      }
      const fileQueue = new UploadQueue(CONCURRENT_FILES);
      const uploadTasks = [];
      for (const fileItem of pendingFiles) {
        fileItem.uploading = true;
        uploadTasks.push(
          fileQueue.add(async () => {
            try {
              const result = await uploadSingleFile(fileItem.file, fileItem.id, pwd);
              uploadResponses.push(result);
              fileItem.uploaded = true;
            } catch (e) {
              uploadFailCount += 1;
              showUploadError('文件 ' + fileItem.file.name + ' 上传失败: ' + (e.message || e));
              fileItem.uploading = false;
            }
          })
        );
      }
      await Promise.all(uploadTasks);
      uploadBtn.disabled = false;
      uploadBtn.textContent = options.uploadBtnLabel || '开始上传';
      if (uploadResponses.length) {
        if (uploadFailCount > 0) {
          showToast('部分文件上传失败（' + uploadFailCount + ' 个）', 'err');
        }
        showResultModal(uploadResponses, uploadFailCount > 0);
      } else if (uploadFailCount > 0) {
        showToast('全部文件上传失败', 'err');
      }
    });

    if (options.closeModalSelector) {
      document.querySelector(options.closeModalSelector)?.addEventListener('click', () => {
        modal.style.display = 'none';
      });
    }
  }

  function computeChunkPlan(totalBytes, chunkBytes) {
    const size = chunkBytes > 0 ? chunkBytes : 20 * 1024 * 1024;
    return { chunkCount: Math.max(1, Math.ceil(totalBytes / size)), chunkBytes: size };
  }

  function showToast(message, type) {
    if (typeof window.TdApi !== 'undefined' && typeof window.TdApi.showToast === 'function') {
      window.TdApi.showToast(message, type);
    } else if (typeof window.showToast === 'function') {
      window.showToast(message, type);
    }
  }

  global.TdUpload = {
    initChunkedUpload,
    getAccessPwd,
    requireLogin,
    formatSize,
    computeChunkPlan,
    newUploadSessionId,
    parseUploadFolderId,
    subscribeUploadProgress,
    fetchWithRetry,
    showToast,
  };
})(window);
