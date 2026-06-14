# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: web-metadata.spec.ts >> Web page metadata >> page-meta.js sets absolute canonical after load
- Location: web-metadata.spec.ts:35:7

# Error details

```
Error: page.goto: Target page, context or browser has been closed
```

# Test source

```ts
  1  | import { test, expect } from '@playwright/test';
  2  | 
  3  | test.describe('Web page metadata', () => {
  4  |   test('login.html has title, description, robots, og:title', async ({ page }) => {
  5  |     await page.goto('/login.html');
  6  |     await expect(page).toHaveTitle(/登录 · Telegram Drive API/i);
  7  |     await expect(page.locator('meta[name="description"]')).toHaveAttribute('content', /.+/);
  8  |     await expect(page.locator('meta[name="robots"]')).toHaveAttribute('content', 'noindex, nofollow');
  9  |     await expect(page.locator('meta[property="og:title"]')).toHaveAttribute('content', /.+/);
  10 |     await expect(page.locator('link[rel="icon"]')).toHaveAttribute('href', '/assets/logo.svg');
  11 |   });
  12 | 
  13 |   test('files.html static head includes metadata (no auth redirect)', async ({ request }) => {
  14 |     const res = await request.get('/files.html');
  15 |     expect(res.ok()).toBeTruthy();
  16 |     const html = await res.text();
  17 |     expect(html).toContain('meta name="description"');
  18 |     expect(html).toContain('meta name="robots" content="noindex, nofollow"');
  19 |     expect(html).toContain('property="og:title"');
  20 |     expect(html).toContain('page-meta.js');
  21 |     expect(html).toContain('文件管理 · Telegram Drive API');
  22 |   });
  23 | 
  24 |   test('docs.html is indexable and has matching og tags', async ({ page }) => {
  25 |     await page.goto('/docs.html');
  26 |     await expect(page).toHaveTitle(/API 文档 · Telegram Drive/i);
  27 |     await expect(page.locator('meta[name="robots"]')).toHaveAttribute('content', 'index, follow');
  28 |     const title = await page.locator('meta[property="og:title"]').getAttribute('content');
  29 |     const desc = await page.locator('meta[property="og:description"]').getAttribute('content');
  30 |     expect(title?.length).toBeGreaterThan(5);
  31 |     expect(desc?.length).toBeGreaterThan(10);
  32 |     expect(await page.title()).toContain(title?.split(' · ')[0] ?? 'API');
  33 |   });
  34 | 
  35 |   test('page-meta.js sets absolute canonical after load', async ({ page }) => {
> 36 |     await page.goto('/login.html', { waitUntil: 'load' });
     |                ^ Error: page.goto: Target page, context or browser has been closed
  37 |     await page.waitForFunction(() => {
  38 |       const link = document.querySelector('link[data-td-canonical]');
  39 |       const href = link?.getAttribute('href') || '';
  40 |       return href.startsWith('http') && href.includes('/login.html');
  41 |     });
  42 |     const canonical = await page.locator('link[data-td-canonical]').getAttribute('href');
  43 |     expect(canonical).toMatch(/^https?:\/\/.+\/login\.html$/);
  44 |     const ogUrl = await page.locator('meta[data-td-og-url]').getAttribute('content');
  45 |     expect(ogUrl).toBe(canonical);
  46 |     const ogImage = await page.locator('meta[data-td-og-image]').getAttribute('content');
  47 |     expect(ogImage).toMatch(/\/assets\/logo\.svg$/);
  48 |   });
  49 | });
  50 | 
```