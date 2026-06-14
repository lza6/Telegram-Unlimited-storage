import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act } from '@testing-library/react';
import { ExternalDropBlocker } from './ExternalDropBlocker';

describe('ExternalDropBlocker', () => {
    beforeEach(() => {
        vi.resetModules();
    });

    afterEach(() => {
        vi.restoreAllMocks();
    });

    it('calls onUploadBlocked when browser drop occurs while upload disabled', async () => {
        const onUploadBlocked = vi.fn();
        render(
            <ExternalDropBlocker
                onUploadPaths={vi.fn()}
                onUploadClick={vi.fn()}
                uploadEnabled={false}
                onUploadBlocked={onUploadBlocked}
            />,
        );

        await act(async () => {
            const event = new Event('drop', { bubbles: true, cancelable: true });
            Object.defineProperty(event, 'dataTransfer', {
                value: { types: ['Files'], files: [] },
            });
            document.dispatchEvent(event);
        });

        expect(onUploadBlocked).toHaveBeenCalledTimes(1);
    });
});
