/** Page metadata contracts — keep deploy/web HTML + page-meta.js in sync. */

export type WebPageId =
    | 'login'
    | 'dashboard'
    | 'files'
    | 'shares'
    | 'settings'
    | 'upload'
    | 'telegram'
    | 'docs'
    | 'index';

export type WebPageMeta = {
    path: string;
    title: string;
    description: string;
    indexable: boolean;
    ogType: 'website' | 'article';
};

const SITE_NAME = 'Telegram Drive';

const PAGE_META: Record<WebPageId, WebPageMeta> = {
    login: {
        path: '/login.html',
        title: '登录 · Telegram Drive API',
        description: 'Telegram Drive 管理登录 — 使用 ACCESS_PWD 访问控制台与 API 服务。',
        indexable: false,
        ogType: 'website',
    },
    dashboard: {
        path: '/dashboard.html',
        title: '控制台 · Telegram Drive API',
        description: 'Telegram Drive 控制台 — 上传队列、服务状态与传输模式概览。',
        indexable: false,
        ogType: 'website',
    },
    files: {
        path: '/files.html',
        title: '文件管理 · Telegram Drive API',
        description: 'Telegram Drive 文件列表 — 浏览、搜索、下载与批量管理索引文件。',
        indexable: false,
        ogType: 'website',
    },
    shares: {
        path: '/shares.html',
        title: '分享管理 · Telegram Drive API',
        description: 'Telegram Drive 分享管理 — 创建、复制与撤销可下载分享链接。',
        indexable: false,
        ogType: 'website',
    },
    settings: {
        path: '/settings.html',
        title: '服务设置 · Telegram Drive API',
        description: 'Telegram Drive 服务设置 — API 密钥、传输模式、代理与分享域名。',
        indexable: false,
        ogType: 'website',
    },
    upload: {
        path: '/upload.html',
        title: '上传页 · Telegram Drive API',
        description: 'Telegram Drive 公共上传页 — 分片上传、进度轮询与文件夹选择。',
        indexable: false,
        ogType: 'website',
    },
    telegram: {
        path: '/telegram.html',
        title: 'Telegram 登录 · Telegram Drive API',
        description: 'Telegram Drive User 会话登录 — 验证码或二维码绑定 GramJS 传输。',
        indexable: false,
        ogType: 'website',
    },
    docs: {
        path: '/docs.html',
        title: 'API 文档 · Telegram Drive',
        description: 'Telegram Drive REST API 与 OpenAPI 规范 — 文件、上传、分享与认证接口。',
        indexable: true,
        ogType: 'article',
    },
    index: {
        path: '/index.html',
        title: 'Telegram Drive API',
        description: 'Telegram Drive API 入口 — 重定向至管理登录。',
        indexable: false,
        ogType: 'website',
    },
};

export function getWebPageMeta(pageId: WebPageId): WebPageMeta {
    return PAGE_META[pageId];
}

export function robotsDirective(indexable: boolean): string {
    return indexable ? 'index, follow' : 'noindex, nofollow';
}

/** Absolute URL for canonical / og:url / og:image (must match at runtime). */
export function resolveAbsoluteUrl(origin: string, pathOrUrl: string): string {
    const base = origin.replace(/\/$/, '');
    const raw = pathOrUrl.trim();
    if (/^https?:\/\//i.test(raw)) {
        return raw;
    }
    const path = raw.startsWith('/') ? raw : `/${raw}`;
    return `${base}${path}`;
}

export function buildOpenGraphBundle(pageId: WebPageId, origin: string) {
    const meta = getWebPageMeta(pageId);
    const canonical = resolveAbsoluteUrl(origin, meta.path);
    const image = resolveAbsoluteUrl(origin, '/assets/logo.svg');
    return {
        siteName: SITE_NAME,
        title: meta.title,
        description: meta.description,
        canonical,
        ogUrl: canonical,
        ogImage: image,
        ogType: meta.ogType,
        robots: robotsDirective(meta.indexable),
        twitterCard: 'summary' as const,
    };
}
