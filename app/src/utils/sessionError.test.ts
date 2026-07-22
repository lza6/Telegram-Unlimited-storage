import { describe, it, expect } from 'vitest';
import { isSessionLostError } from './sessionError';

describe('isSessionLostError', () => {
    it('detects session-specific errors', () => {
        expect(isSessionLostError('Telegram client is not connected')).toBe(true);
        expect(isSessionLostError('Session expired — sign in again')).toBe(true);
    });

    it('does not logout on generic network timeout', () => {
        expect(isSessionLostError('network timeout')).toBe(false);
        expect(isSessionLostError('ECONNREFUSED')).toBe(false);
    });

    it('does not match broad auth substring in unrelated errors', () => {
        expect(isSessionLostError('authorization header missing for unrelated service')).toBe(false);
    });
});
