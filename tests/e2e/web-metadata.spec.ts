import { test, expect } from '@playwright/test';

test.describe('Web page metadata', () => {
  test('login.html has title, description, robots, og:title', async ({ page }) => {
    await page.goto('/login.html');
    await expect(page).toHaveTitle(/登录 · Telegram Drive API/i);
    await expect(page.locator('meta[name="description"]')).toHaveAttribute('content', /.+/);
    await expect(page.locator('meta[name="robots"]')).toHaveAttribute('content', 'noindex, nofollow');
    await expect(page.locator('meta[property="og:title"]')).toHaveAttribute('content', /.+/);
    await expect(page.locator('link[rel="icon"]')).toHaveAttribute('href', '/assets/logo.svg');
  });

  test('files.html static head includes metadata (no auth redirect)', async ({ request }) => {
    const res = await request.get('/files.html');
    expect(res.ok()).toBeTruthy();
    const html = await res.text();
    expect(html).toContain('meta name="description"');
    expect(html).toContain('meta name="robots" content="noindex, nofollow"');
    expect(html).toContain('property="og:title"');
    expect(html).toContain('page-meta.js');
    expect(html).toContain('文件管理 · Telegram Drive API');
  });

  test('docs.html is indexable and has matching og tags', async ({ page }) => {
    await page.goto('/docs.html');
    await expect(page).toHaveTitle(/API 文档 · Telegram Drive/i);
    await expect(page.locator('meta[name="robots"]')).toHaveAttribute('content', 'index, follow');
    const title = await page.locator('meta[property="og:title"]').getAttribute('content');
    const desc = await page.locator('meta[property="og:description"]').getAttribute('content');
    expect(title?.length).toBeGreaterThan(5);
    expect(desc?.length).toBeGreaterThan(10);
    expect(await page.title()).toContain(title?.split(' · ')[0] ?? 'API');
  });

  test('page-meta.js sets absolute canonical after load', async ({ page }) => {
    await page.goto('/login.html', { waitUntil: 'load' });
    await page.waitForFunction(() => {
      const link = document.querySelector('link[data-td-canonical]');
      const href = link?.getAttribute('href') || '';
      return href.startsWith('http') && href.includes('/login.html');
    });
    const canonical = await page.locator('link[data-td-canonical]').getAttribute('href');
    expect(canonical).toMatch(/^https?:\/\/.+\/login\.html$/);
    const ogUrl = await page.locator('meta[data-td-og-url]').getAttribute('content');
    expect(ogUrl).toBe(canonical);
    const ogImage = await page.locator('meta[data-td-og-image]').getAttribute('content');
    expect(ogImage).toMatch(/\/assets\/logo\.svg$/);
  });
});
