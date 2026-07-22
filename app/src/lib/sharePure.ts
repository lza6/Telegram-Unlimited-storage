/** Share UX helpers — mirrored in deploy/web/assets/share-pure.js */

export function formatShareCreateErrorMessage(err: unknown): string {
    const msg = err != null ? String(err) : '';
    if (!msg) return '创建分享失败';
    if (msg.includes('bot_file_map') || msg.includes('Bot download')) {
        return '该文件尚未建立 Bot 下载映射，无法创建分享。请先在 Bot 模式下通过 Bot 上传，或在设置中重建/同步索引后再试。';
    }
    if (msg.includes('asset index')) {
        return '该文件不在资产索引中，无法创建分享。请先在设置中重建文件索引。';
    }
    if (msg.includes('Access denied') || msg.includes('another tenant')) {
        return '无权为该文件创建分享（租户隔离）。';
    }
    return msg;
}
