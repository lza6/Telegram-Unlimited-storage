/* Telegram Drive PWA Service Worker — v2.0
 * Cache-first for static assets, network-first for API, stale-while-revalidate
 * for page navigations. Install caches the app shell so it loads offline.
 */
const CACHE_VERSION = 'td-v2-001';
const STATIC_CACHE = 'td-static-' + CACHE_VERSION;
const PAGE_CACHE = 'td-pages-' + CACHE_VERSION;
const API_CACHE = 'td-api-' + CACHE_VERSION;

const APP_SHELL = [
  '/dashboard.html',
  '/files.html',
  '/shares.html',
  '/settings.html',
  '/upload.html',
  '/docs.html',
  '/telegram.html',
  '/login.html',
  '/index.html',
  '/assets/admin.css',
  '/assets/theme.js',
  '/assets/api-client.js',
  '/assets/notifications.js',
  '/assets/logo.svg',
  '/manifest.json',
];

// ── Install: pre-cache the app shell ──────────────────────────────────────
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(STATIC_CACHE).then((cache) => {
      return Promise.allSettled(
        APP_SHELL.map((url) =>
          cache.add(url).catch(() => {
            /* skip missing assets gracefully */
          }),
        ),
      );
    }).then(() => self.skipWaiting()),
  );
});

// ── Activate: clean old caches ────────────────────────────────────────────
self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(
        keys
          .filter((k) => k.startsWith('td-') && k !== STATIC_CACHE && k !== PAGE_CACHE && k !== API_CACHE)
          .map((k) => caches.delete(k)),
      ),
    ).then(() => self.clients.claim()),
  );
});

// ── Fetch: route by request type ──────────────────────────────────────────
self.addEventListener('fetch', (event) => {
  const { request } = event;
  const url = new URL(request.url);

  // Same-origin only
  if (url.origin !== self.location.origin) return;

  // Skip non-GET
  if (request.method !== 'GET') return;

  // API requests: Network First → cache fallback
  if (url.pathname.startsWith('/api/')) {
    event.respondWith(networkFirst(request, API_CACHE));
    return;
  }

  // Static assets (CSS, JS, images, fonts): Cache First
  if (
    request.destination === 'style' ||
    request.destination === 'script' ||
    request.destination === 'image' ||
    request.destination === 'font' ||
    url.pathname.startsWith('/assets/')
  ) {
    event.respondWith(cacheFirst(request, STATIC_CACHE));
    return;
  }

  // Page navigations: Network First (stale-while-revalidate style)
  if (request.mode === 'navigate' || request.destination === 'document') {
    event.respondWith(networkFirst(request, PAGE_CACHE));
    return;
  }

  // Everything else: Network First
  event.respondWith(networkFirst(request, STATIC_CACHE));
});

// ── Strategy: Cache First (static assets that rarely change) ──────────────
async function cacheFirst(request, cacheName) {
  const cached = await caches.match(request);
  if (cached) return cached;
  try {
    const response = await fetch(request);
    if (response.ok) {
      const cache = await caches.open(cacheName);
      cache.put(request, response.clone());
    }
    return response;
  } catch (_err) {
    // Offline + not cached → return a simple offline indicator
    return new Response('Offline', { status: 503, statusText: 'Service Unavailable' });
  }
}

// ── Strategy: Network First (API / dynamic content) ───────────────────────
async function networkFirst(request, cacheName) {
  try {
    const response = await fetch(request);
    if (response.ok) {
      const cache = await caches.open(cacheName);
      cache.put(request, response.clone());
    }
    return response;
  } catch (_err) {
    const cached = await caches.match(request);
    return cached || new Response('Offline', { status: 503, statusText: 'Service Unavailable' });
  }
}

// ── Push notification handler (placeholder) ────────────────────────────────
self.addEventListener('push', (event) => {
  const data = event.data ? event.data.json() : {};
  event.waitUntil(
    self.registration.showNotification(data.title || 'Telegram Drive', {
      body: data.body || '',
      icon: '/assets/logo.svg',
      badge: '/assets/logo.svg',
      data: { url: data.url || '/dashboard.html' },
    }),
  );
});

self.addEventListener('notificationclick', (event) => {
  event.notification.close();
  event.waitUntil(clients.openWindow(event.notification.data.url));
});

// ── Background Sync: replay failed uploads when connectivity returns ───────
// (TASK-P1-02, v8.0)
//
// Clients register the 'upload-queue' sync tag via registration.sync.register()
// after an upload fails offline. On the sync event, we drain the IndexedDB
// 'upload_queue' store and replay each pending upload, dispatching a td:toast
// event to the controlling client on success/failure.
const SYNC_TAG = 'upload-queue';
const DB_NAME = 'td-offline';
const DB_VERSION = 1;
const STORE = 'upload_queue';

function openOfflineDB() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = function (ev) {
      const db = ev.target.result;
      if (!db.objectStoreNames.contains(STORE)) {
        db.createObjectStore(STORE, { keyPath: 'id', autoIncrement: true });
      }
    };
    req.onsuccess = function () { resolve(req.result); };
    req.onerror = function () { reject(req.error); };
  });
}

self.addEventListener('sync', (event) => {
  if (event.tag === SYNC_TAG) {
    event.waitUntil(replayUploadQueue());
  }
});

async function replayUploadQueue() {
  let db;
  try {
    db = await openOfflineDB();
  } catch (_err) {
    return; // IndexedDB unavailable — nothing to replay
  }
  const tx = db.transaction(STORE, 'readonly');
  const store = tx.objectStore(STORE);
  const all = await new Promise((resolve, reject) => {
    const r = store.getAll();
    r.onsuccess = function () { resolve(r.result || []); };
    r.onerror = function () { reject(r.error); };
  });

  for (const item of all) {
    try {
      const res = await fetch(item.url, {
        method: item.method || 'POST',
        headers: item.headers || {},
        body: item.body,
        credentials: 'include',
      });
      if (res.ok) {
        await deleteFromQueue(db, item.id);
        await notifyClients('success', '离线上传已同步: ' + (item.name || ''));
      } else {
        // Non-2xx — leave in queue for next sync; avoid hot-looping.
        await notifyClients('warning', '离线上传待重试: ' + (item.name || ''));
        break;
      }
    } catch (_err) {
      await notifyClients('error', '离线上传失败: ' + (item.name || ''));
      break; // still offline — stop, will retry on next sync
    }
  }
}

function deleteFromQueue(db, id) {
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE, 'readwrite');
    tx.objectStore(STORE).delete(id);
    tx.oncomplete = function () { resolve(); };
    tx.onerror = function () { reject(tx.error); };
  });
}

async function notifyClients(type, message) {
  const clients = await self.clients.matchAll({ includeUncontrolled: true });
  for (const client of clients) {
    client.postMessage({ kind: 'td:toast', type: type, message: message });
  }
}