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
});
