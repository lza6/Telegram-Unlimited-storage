import { describe, expect, it } from 'vitest';
import { formatShareCreateErrorMessage } from './sharePure';

describe('formatShareCreateErrorMessage', () => {
    it('maps missing bot_file_map to actionable Chinese', () => {
        expect(
            formatShareCreateErrorMessage(
                'File is not registered for Bot download (missing bot_file_map)',
            ),
        ).toContain('Bot 下载映射');
    });

    it('maps asset index errors', () => {
        expect(
            formatShareCreateErrorMessage('File is not registered in the asset index'),
        ).toContain('重建文件索引');
    });

    it('passes through unknown errors', () => {
        expect(formatShareCreateErrorMessage('Network timeout')).toBe('Network timeout');
    });
});
