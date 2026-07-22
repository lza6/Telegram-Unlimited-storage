import { describe, it, expect } from 'vitest';
import {
    canDownloadFiles,
    canPreviewFiles,
    canShareFiles,
    canTransferFiles,
    classifyConnectionStatus,
    connectionStatusLabel,
    isBotIndexReady,
    isBotTransportMode,
    isServiceReady,
} from './connection';

describe('connection status', () => {
    it('only online allows transfers', () => {
        expect(canTransferFiles('online')).toBe(true);
        expect(canTransferFiles('checking')).toBe(false);
        expect(canTransferFiles('session_lost')).toBe(false);
        expect(canTransferFiles('network_offline')).toBe(false);
    });

    it('labels checking state', () => {
        expect(connectionStatusLabel('checking')).toContain('Checking');
    });

    it('classifyConnectionStatus respects network and telegram', () => {
        expect(classifyConnectionStatus(false, true)).toBe('network_offline');
        expect(classifyConnectionStatus(true, true)).toBe('online');
        expect(classifyConnectionStatus(true, false)).toBe('session_lost');
    });

    it('labels all connection states', () => {
        expect(connectionStatusLabel('online')).toContain('active');
        expect(connectionStatusLabel('session_lost')).toContain('expired');
        expect(connectionStatusLabel('network_offline')).toContain('network');
    });

    it('isServiceReady when API health ready without GramJS', () => {
        expect(isServiceReady({ connectionStatus: 'session_lost', apiHealthReady: true })).toBe(true);
        expect(isServiceReady({ connectionStatus: 'session_lost' })).toBe(false);
        expect(isServiceReady({ connectionStatus: 'online' })).toBe(true);
    });

    it('canDownloadFiles mirrors transfer or bot index', () => {
        expect(canDownloadFiles({ transferReady: true })).toBe(true);
        expect(canDownloadFiles({ transferReady: false, botIndexReady: true })).toBe(true);
        expect(canDownloadFiles({ transferReady: false, botIndexReady: false })).toBe(false);
    });

    it('canPreviewFiles mirrors canDownloadFiles', () => {
        expect(canPreviewFiles({ transferReady: true })).toBe(true);
        expect(canPreviewFiles({ transferReady: false, botIndexReady: true })).toBe(true);
        expect(canPreviewFiles({ transferReady: false, botIndexReady: false })).toBe(false);
    });

    it('canShareFiles mirrors canDownloadFiles', () => {
        expect(canShareFiles({ transferReady: true })).toBe(true);
        expect(canShareFiles({ transferReady: false, botIndexReady: true })).toBe(true);
        expect(canShareFiles({ transferReady: false, botIndexReady: false })).toBe(false);
    });

    it('isBotIndexReady requires bot transport and API ready', () => {
        expect(isBotTransportMode('bot')).toBe(true);
        expect(isBotTransportMode('user')).toBe(false);
        expect(isBotIndexReady({ apiHealthReady: true, transportMode: 'bot' })).toBe(true);
        expect(isBotIndexReady({ apiHealthReady: true, transportMode: 'user' })).toBe(false);
        expect(isBotIndexReady({ transportMode: 'bot' })).toBe(false);
    });
});
