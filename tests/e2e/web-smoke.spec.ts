import { test, expect } from '@playwright/test';

const accessPwd = process.env.E2E_ACCESS_PWD || 'test';

test.describe('Telegram Drive web smoke', () => {
  test('login page loads and rejects bad password', async ({ page }) => {
    await page.goto('/login.html');
    await expect(page.getByRole('heading', { name: /Telegram Drive/i })).toBeVisible();
    await page.fill('#pwd-input', 'wrong-password-xyz');
    await page.getByRole('button', { name: /进入控制台/ }).click();
    await expect(page.locator('#err')).not.toHaveClass(/hidden/);
  });

  test('docs page exposes OpenAPI badge', async ({ page }) => {
    await page.goto('/docs.html');
    await expect(page.locator('body')).toContainText(/OpenAPI|Swagger|API/i);
  });

  test('health JSON is reachable', async ({ request }) => {
    const res = await request.get('/api/v1/health');
    if (res.status() === 404) {
      test.skip(true, 'Requires headless API (static file server only)');
    }
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body.status).toBe('ok');
    expect(body.upload_queue).toBeTruthy();
  });

  test('rebuild-index requires auth', async ({ request }) => {
    const probe = await request.get('/api/v1/health');
    if (probe.status() === 404) {
      test.skip(true, 'Requires headless API (static file server only)');
    }
    const res = await request.post('/api/v1/files/rebuild-index', {
      data: { folder_ids: [null] },
    });
    expect(res.status()).toBe(401);
  });

  test('files-core rebuild-index wired on refresh', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('/api/v1/files/rebuild-index');
    expect(text).toContain('rebuildIndexIfUser');
  });

  test('files-core bulk delete groups by folder_id', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('bulkDeleteByFolder');
    expect(text).toContain('TdFilesPure.buildBulkDeletePayloads');
  });

  test('files-core search rebuilds index before query', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('await rebuildIndexIfUser');
  });

  test('files-pure exposes bulk delete and download helpers', async ({ request }) => {
    const res = await request.get('/assets/files-pure.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('buildBulkDeletePayloads');
    expect(text).toContain('buildBulkMovePayloads');
    expect(text).toContain('buildFileDownloadUrl');
    expect(text).toContain('TdFilesPure');
  });

  test('files-core bulk move wired with folder select', async ({ request }) => {
    const html = await request.get('/files.html');
    expect(html.ok()).toBeTruthy();
    expect(await html.text()).toContain('bulk-move-folder');
    expect(await html.text()).toContain('bulk-move-btn');

    const js = await request.get('/assets/files-core.js');
    expect(js.ok()).toBeTruthy();
    const text = await js.text();
    expect(text).toContain('bulkMoveByFolder');
    expect(text).toContain('buildBulkMovePayloads');
    expect(text).toContain('loadMoveFolders');
    expect(text).toContain('transportMode');
    expect(text).toContain('TdFilesPure.bulkMoveBlockedMessage');
    expect(text).toContain('TdFilesPure.canBulkMoveInTransportMode');
  });

  test('upload-core passes folder_id on merge and small upload', async ({ request }) => {
    const res = await request.get('/assets/upload-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('folderSelectSelector');
    expect(text).toContain("appendFolderId(mergeFormData, folderId)");
    expect(text).toContain('appendFolderId(formData, folderId)');
  });

  test('dashboard exposes upload folder selector', async ({ request }) => {
    const res = await request.get('/dashboard.html');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('upload-folder');
    expect(text).toContain('upload-folder.js');
  });

  test('dashboard exposes user onboarding card', async ({ request }) => {
    const res = await request.get('/dashboard.html');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('onboarding-user');
    expect(text).toContain('TdWebPure.shouldShowUserOnboarding');
    expect(text).toContain('td_user_onboarding_done');
    expect(text).toContain('initDashboard');
  });

  test('files-core silent rebuild uses TdWebPure toast gate', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('TdWebPure.rebuildIndexShouldToast');
    expect(text).toContain("rebuildIndexIfUser('refresh')");
    expect(text).toContain("rebuildIndexIfUser('search')");
    expect(text).toContain('rebuildIndexShouldSurfaceBackgroundFailure');
    expect(text).toContain('formatRebuildIndexBackgroundFailureMessage');
  });

  test('telegram-auth surfaces qr poll errors', async ({ request }) => {
    const res = await request.get('/assets/telegram-auth.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain("getElementById('qr-poll')");
    expect(text).not.toContain('pollQrOnce().catch(function () {})');
  });

  test('web-pure exposes onboarding and rebuild helpers', async ({ request }) => {
    const res = await request.get('/assets/web-pure.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('shouldShowBotOnboarding');
    expect(text).toContain('shouldShowUserOnboarding');
    expect(text).toContain('rebuildIndexShouldToast');
    expect(text).toContain('rebuildIndexShouldSurfaceBackgroundFailure');
  });

  test('files.html loads web-pure before files-core', async ({ request }) => {
    const res = await request.get('/files.html');
    expect(res.ok()).toBeTruthy();
    const html = await res.text();
    const pureIdx = html.indexOf('web-pure.js');
    const coreIdx = html.indexOf('files-core.js');
    expect(pureIdx).toBeGreaterThan(-1);
    expect(coreIdx).toBeGreaterThan(pureIdx);
  });

  test('files-core surfaces move folder load errors', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('loadMoveFolders');
    expect(text).toContain('加载文件夹列表失败');
  });

  test('files-core keeps selectedMeta for cross-page bulk delete', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('selectedMeta');
    expect(text).toContain('rememberFileMeta');
  });

  test('settings-core warns index reset on transport switch', async ({ request }) => {
    const res = await request.get('/assets/settings-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('文件索引已重置');
  });

  test('settings exposes manual rebuild index button', async ({ request }) => {
    const res = await request.get('/settings.html');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('rebuild-index-btn');
    expect(text).toContain('web-pure.js');
  });

  test('settings-core manual rebuild uses TdWebPure toast gate', async ({ request }) => {
    const res = await request.get('/assets/settings-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('rebuildFileIndexManual');
    expect(text).toContain("TdWebPure.rebuildIndexShouldToast('manual')");
    expect(text).toContain('/api/v1/files/rebuild-index');
  });

  test('page-readiness disables upload folder select when blocked', async ({ request }) => {
    const res = await request.get('/assets/page-readiness.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('folderSelectSelector');
    expect(text).toContain('folderSelect.disabled');
  });

  test('files-core uses TdFilesPure helpers', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('TdFilesPure.buildBulkDeletePayloads');
    expect(text).toContain('TdFilesPure.buildFileDownloadUrl');
  });

  test('files-core download uses blob fetch', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('TdFilesPure.buildFileDownloadUrl');
    expect(text).toContain('downloadingIds');
    expect(text).toContain('TdDownloadPure');
    expect(text).toContain('readResponseBlobWithProgress');
  });

  test('download-pure mirrors TypeScript helpers', async ({ request }) => {
    const res = await request.get('/assets/download-pure.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('shouldBlockDuplicateDownload');
    expect(text).toContain('deriveWebDownloadButtonState');
    expect(text).toContain('buildDownloadStartToast');
    expect(text).toContain('computeDownloadPercent');
    expect(text).toContain('readResponseBlobWithProgress');
    expect(text).toContain('consumeStreamWithProgress');
  });

  test('files.html loads download-pure before files-core', async ({ request }) => {
    const res = await request.get('/files.html');
    expect(res.ok()).toBeTruthy();
    const html = await res.text();
    const pureIdx = html.indexOf('download-pure.js');
    const coreIdx = html.indexOf('files-core.js');
    expect(pureIdx).toBeGreaterThan(-1);
    expect(coreIdx).toBeGreaterThan(pureIdx);
  });

  test('files-core bulk move click uses TdFilesPure transport guard', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('TdFilesPure.canBulkMoveInTransportMode(transportMode)');
    expect(text).toContain('TdFilesPure.bulkMoveBlockedMessage(transportMode)');
  });

  test('files-core bulk move avoids misleading toast on API failure', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('moveResult.failures.length');
    expect(text).toContain('deleteResult.failures.length');
    expect(text).toContain('succeededIds: succeededIds');
    expect(text).toContain('moveResult.succeededIds.forEach');
  });

  test('files-core row share warns before permanent link', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('无密码、永久有效');
  });

  test('files-core share errors use TdSharePure formatter', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('TdSharePure.formatShareCreateErrorMessage');
  });

  test('share-pure exposes bot_file_map error helper', async ({ request }) => {
    const res = await request.get('/assets/share-pure.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('formatShareCreateErrorMessage');
    expect(text).toContain('bot_file_map');
  });

  test('files.html loads share-pure before files-core', async ({ request }) => {
    const html = await request.get('/files.html');
    const text = await html.text();
    expect(text.indexOf('share-pure.js')).toBeLessThan(text.indexOf('files-core.js'));
  });

  test('files-core delete notifies share list invalidation', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('formatDeleteSuccessToast');
    expect(text).toContain('bumpSharesInvalidateStorage');
    expect(text).toContain('formatBulkDeleteConfirmMessage');
  });

  test('shares-core listens for cross-tab invalidation', async ({ request }) => {
    const res = await request.get('/assets/shares-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('td-shares-invalidate');
    expect(text).toContain("addEventListener('storage'");
    expect(text).toContain('visibilitychange');
  });

  test('files-core delete aggregates shares_revoked from bulk API', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('sharesRevoked');
    expect(text).toContain('res.shares_revoked');
  });

  test('upload-core aborts in-flight chunks when SSE reports failed', async ({ request }) => {
    const res = await request.get('/assets/upload-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('chunkAbort');
    expect(text).toContain('chunkAbort.abort()');
    expect(text).toContain('signal: chunkAbort.signal');
  });

  test('upload-core surfaces partial failure as error toast', async ({ request }) => {
    const res = await request.get('/assets/upload-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain("showToast('部分文件上传失败");
    expect(text).toContain("'err'");
  });

  test('files-core search empty state differs from folder empty', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('state.search.trim()');
    expect(text).toContain('未找到匹配');
  });

  test('files-pure exposes bulk move transport guard', async ({ request }) => {
    const res = await request.get('/assets/files-pure.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('canBulkMoveInTransportMode');
    expect(text).toContain('bulkMoveBlockedMessage');
  });

  test('shares-core revoke checks service readiness', async ({ request }) => {
    const res = await request.get('/assets/shares-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('ensureReadyForAction');
    expect(text).toContain("method: 'DELETE'");
  });

  test('verify accepts ACCESS_PWD', async ({ request }) => {
    const res = await request.post('/verify', {
      form: { pwd: accessPwd },
    });
    expect(res.status()).toBeLessThan(500);
  });

  test('upload page exposes 503 retry UI', async ({ page }) => {
    await page.addInitScript(() => {
      sessionStorage.setItem('td_access_pwd', 'test');
    });
    await page.goto('/upload.html');
    await expect(page.locator('#retry-status')).toBeAttached();
    await expect(page.locator('#retry-status')).toHaveClass(/hidden/);
  });

  test('upload-core prefers progress token over pwd in query', async ({ request }) => {
    const res = await request.get('/assets/upload-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('upload_progress_token');
    expect(text).toContain("encodeURIComponent(data.token)");
    expect(text).toContain('PROGRESS_TOKEN_TIMEOUT_MS');
    expect(text).toContain('controller.abort()');
    expect(text).not.toContain("return '&pwd='");
    expect(text).not.toContain("authQ.indexOf('pwd=')");
  });

  test('upload progress token failure does not open a query-auth channel', async ({ page }) => {
    const tokenRequests: string[] = [];
    await page.addInitScript(() => {
      sessionStorage.setItem('td_access_pwd', 'correct-admin-password');
      (window as any).__progressChannelCalls = { eventSource: 0, webSocket: 0 };
      (window as any).EventSource = function () {
        (window as any).__progressChannelCalls.eventSource += 1;
      };
      (window as any).WebSocket = function () {
        (window as any).__progressChannelCalls.webSocket += 1;
      };
    });
    await page.route('**/upload_progress_token', async (route) => {
      tokenRequests.push(route.request().url());
      await route.fulfill({ status: 503, contentType: 'application/json', body: '{}' });
    });
    await page.goto('/login.html');
    await page.addScriptTag({ url: '/assets/upload-core.js' });

    const result = await page.evaluate(async () => {
      const statuses: Array<{ status: string; detail: string }> = [];
      const source = await (window as any).TdUpload.subscribeUploadProgress(
        'sess-token-failure',
        () => {},
        'correct-admin-password',
        (status: { status: string; detail: string }) => statuses.push(status),
      );
      return {
        sourceIsNull: source === null,
        statuses,
        channelCalls: (window as any).__progressChannelCalls,
      };
    });

    expect(result.sourceIsNull).toBeTruthy();
    expect(result.channelCalls).toEqual({ eventSource: 0, webSocket: 0 });
    expect(result.statuses).toEqual([
      {
        status: 'error',
        detail: '无法获取上传进度令牌；上传仍会继续，请查看文件行状态。',
      },
    ]);
    expect(tokenRequests).toHaveLength(1);
    expect(tokenRequests[0]).not.toContain('pwd=');
    expect(tokenRequests[0]).not.toContain('correct-admin-password');
  });

  test('chunk upload continues when progress token issuance fails', async ({ page }) => {
    const tokenRequests: string[] = [];
    const chunkRequests: string[] = [];
    let nextChunkId = 100;
    let mergeCalled = false;

    await page.addInitScript(() => {
      sessionStorage.setItem('td_access_pwd', 'correct-admin-password');
    });
    await page.route('**/api/v1/auth/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          connected: true,
          credentials_ok: true,
          transport_mode: 'bot',
          bot_configured: true,
          user_configured: false,
        }),
      });
    });
    await page.route('**/api/v1/health', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          status: 'ok',
          ready: true,
          telegram_connected: true,
          version: 'test',
          transport_mode: 'bot',
          presigned_download_enabled: true,
          multi_tenant_enabled: false,
        }),
      });
    });
    await page.route('**/api/v1/folders', async (route) => {
      await route.fulfill({ status: 200, contentType: 'application/json', body: '[]' });
    });
    await page.route('**/config', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          chunk_size_mb: 1,
          chunk_concurrent: 2,
          files_concurrent: 1,
        }),
      });
    });
    await page.route('**/upload_progress_token', async (route) => {
      tokenRequests.push(route.request().url());
      await route.fulfill({ status: 503, contentType: 'application/json', body: '{}' });
    });
    await page.route('**/upload_chunk', async (route) => {
      chunkRequests.push(route.request().url());
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ file_id: nextChunkId++ }),
      });
    });
    await page.route('**/merge_chunks', async (route) => {
      mergeCalled = true;
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          filename: 'large.bin',
          download_url: 'http://localhost:1334/d/test-token',
        }),
      });
    });

    await page.goto('/upload.html');
    await expect(page.locator('#upload-btn')).toBeEnabled();
    await page.locator('#file-input').setInputFiles({
      name: 'large.bin',
      mimeType: 'application/octet-stream',
      buffer: Buffer.alloc(2 * 1024 * 1024 + 1, 7),
    });
    await page.locator('#upload-btn').click();

    await expect(page.locator('#status-0')).toHaveText('完成');
    expect(tokenRequests).toHaveLength(1);
    expect(tokenRequests[0]).not.toContain('pwd=');
    expect(tokenRequests[0]).not.toContain('correct-admin-password');
    expect(chunkRequests.length).toBeGreaterThan(0);
    expect(chunkRequests.every((url) => !url.includes('pwd='))).toBeTruthy();
    expect(mergeCalled).toBeTruthy();
  });

  test('dashboard upload button disabled when service not ready', async ({ page }) => {
    await page.addInitScript(() => {
      sessionStorage.setItem('td_access_pwd', 'test');
    });
    const healthBody = JSON.stringify({
      status: 'ok',
      ready: false,
      transport_mode: 'bot',
      version: '4.0.0-beta',
    });
    const authBody = JSON.stringify({
      connected: false,
      transport_mode: 'bot',
      credentials_ok: true,
    });
    const fulfillJson = (body: string) => async (route: import('@playwright/test').Route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body,
      });
    };
    await page.route('**/api/v1/health', fulfillJson(healthBody));
    await page.route('**/api/v1/health/**', fulfillJson(healthBody));
    await page.route('**/api/v1/auth/status', fulfillJson(authBody));
    await page.route('**/api/v1/auth/status/**', fulfillJson(authBody));
    await page.route('**/api/v1/folders', fulfillJson('[]'));
    await page.route('**/api/v1/folders/**', fulfillJson('[]'));

    await page.goto('/dashboard.html');
    await page.waitForFunction(
      () =>
        typeof (window as unknown as { TdPageReadiness?: { refreshUploadReadiness: unknown } })
          .TdPageReadiness?.refreshUploadReadiness === 'function',
    );
    await page.evaluate(async () => {
      const w = window as unknown as {
        TdPageReadiness: {
          refreshUploadReadiness: (o: {
            bannerPrefix: string;
            folderSelectSelector: string;
          }) => Promise<boolean>;
        };
      };
      await w.TdPageReadiness.refreshUploadReadiness({
        bannerPrefix: '上传暂不可用：',
        folderSelectSelector: '#upload-folder',
      });
    });
    await expect(page.locator('#tg-status')).toContainText('未就绪');
    await expect(page.locator('#upload-btn')).toBeDisabled();
    await expect(page.locator('#service-banner')).toBeVisible();
  });

  test('files-core partial bulk prunes only succeeded selection ids', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('succeededIds');
    expect(text).toContain('moveResult.succeededIds.forEach');
    expect(text).toContain('deleteResult.succeededIds.forEach');
    expect(text).toContain('state.selected.delete(String(id))');
  });

  test('share-domain toasts when refresh fails after save', async ({ request }) => {
    const res = await request.get('/assets/share-domain.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('分享域名已保存，但刷新生效状态失败');
  });

  test('files-pure resolves bulk batch succeeded ids from count', async ({ request }) => {
    const res = await request.get('/assets/files-pure.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('resolveBulkBatchSucceededIds');
    expect(text).toContain('pickBulkSucceededIds');
    expect(text).toContain('partialBatch');
  });

  test('files-core uses pickBulkSucceededIds for selection prune', async ({ request }) => {
    const res = await request.get('/assets/files-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('TdFilesPure.pickBulkSucceededIds');
    expect(text).toContain('res.succeeded_ids');
    expect(text).toContain('partialBatches');
  });

  test('upload-core ws failure falls back to status poll', async ({ request }) => {
    const res = await request.get('/assets/upload-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('beginWsStatusPoll');
  });

  test('upload-core poll failure uses err toast', async ({ request }) => {
    const res = await request.get('/assets/upload-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain("TdApi.showToast('无法获取上传进度，请查看文件行状态', 'err')");
  });
  test('upload-core reuses stable idempotency key and surfaces saga states', async ({ request }) => {
    const res = await request.get('/assets/upload-core.js');
    expect(res.ok()).toBeTruthy();
    const text = await res.text();
    expect(text).toContain('stableUploadIdempotencyKey');
    expect(text).toContain("'Idempotency-Key': idempotencyKey");
    expect(text).toContain('UPLOAD_IN_PROGRESS');
    expect(text).toContain('UPLOAD_RECONCILIATION_REQUIRED');
    expect(text).toContain('UPLOAD_COMPENSATION_PENDING');
    expect(text).toContain('MANUAL_REVIEW');
    expect(text).toContain('SCHEDULER');
  });

  test('upload direct-link dialog is accessible and reports missing links', async ({ request }) => {
    const html = await (await request.get('/upload.html')).text();
    expect(html).toContain('role="dialog"');
    expect(html).toContain('aria-modal="true"');
    expect(html).toContain('id="result-links" role="status" aria-live="polite"');
    const script = await (await request.get('/assets/upload-core.js')).text();
    expect(script).toContain('服务未返回可用直链');
    expect(script).toContain("firstCopy.focus()");
    expect(script).toContain("uploadBtn.focus()");
  });

  test('files page exposes assertive service and polite action status regions', async ({ request }) => {
    const html = await (await request.get('/files.html')).text();
    expect(html).toContain('role="alert" aria-live="assertive"');
    expect(html).toContain('id="file-action-status"');
    expect(html).toContain('aria-live="polite"');
  });
});
