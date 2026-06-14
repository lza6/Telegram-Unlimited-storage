import { describe, it, expect } from 'vitest';
import {
    buildOpenGraphBundle,
    getWebPageMeta,
    resolveAbsoluteUrl,
    robotsDirective,
} from './webMetaPure';

describe('webMetaPure', () => {
    it('admin pages are noindex', () => {
        expect(robotsDirective(getWebPageMeta('login').indexable)).toBe('noindex, nofollow');
        expect(robotsDirective(getWebPageMeta('files').indexable)).toBe('noindex, nofollow');
    });

    it('docs page is indexable', () => {
        expect(robotsDirective(getWebPageMeta('docs').indexable)).toBe('index, follow');
    });

    it('title and description agree in OG bundle', () => {
        const bundle = buildOpenGraphBundle('docs', 'https://drive.example.com');
        expect(bundle.title).toBe(getWebPageMeta('docs').title);
        expect(bundle.description).toBe(getWebPageMeta('docs').description);
        expect(bundle.canonical).toBe('https://drive.example.com/docs.html');
        expect(bundle.ogUrl).toBe(bundle.canonical);
    });

    it('resolveAbsoluteUrl normalizes paths', () => {
        expect(resolveAbsoluteUrl('https://a.test', '/files.html')).toBe('https://a.test/files.html');
        expect(resolveAbsoluteUrl('https://a.test/', 'assets/logo.svg')).toBe('https://a.test/assets/logo.svg');
    });
});
