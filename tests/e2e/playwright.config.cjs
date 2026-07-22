const path = require('node:path');
const { defineConfig } = require('@playwright/test');

const rootDir = __dirname;
const repoRoot = path.resolve(rootDir, '../..');
const webRoot = path.resolve(repoRoot, 'deploy/web');
const headlessScript = path.resolve(repoRoot, 'scripts/e2e-headless-server.ps1');
const useApiServer = process.env.E2E_API === '1' || process.env.E2E_API === 'true';

module.exports = defineConfig({
  testDir: '.',
  timeout: 60_000,
  use: {
    baseURL: process.env.E2E_BASE_URL || 'http://localhost:1334',
    headless: true,
  },
  webServer: process.env.E2E_BASE_URL
    ? undefined
    : useApiServer
      ? {
          command: `powershell -NoProfile -ExecutionPolicy Bypass -File "${headlessScript}" -RepoRoot "${repoRoot}" -Port 1334`,
          url: 'http://127.0.0.1:1334/health/live',
          reuseExistingServer: false,
          timeout: 300_000,
        }
      : {
          command: `node "${path.join(rootDir, 'node_modules/serve/build/main.js')}" -l 1334 "${webRoot}"`,
          cwd: rootDir,
          url: 'http://localhost:1334/login.html',
          reuseExistingServer: true,
          timeout: 120_000,
        },
});
