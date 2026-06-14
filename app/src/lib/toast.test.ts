import { describe, it, expect, vi, beforeEach } from 'vitest';
import { showToast, dismissToast, promiseToast } from './toast';
import * as sonner from 'sonner';

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(() => 'success-id'),
    error: vi.fn(() => 'error-id'),
    loading: vi.fn(() => 'loading-id'),
    info: vi.fn(() => 'info-id'),
    dismiss: vi.fn(),
    promise: vi.fn((p) => p),
  },
}));

describe('showToast', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('calls success toast with message', () => {
    const id = showToast({ type: 'success', message: '操作成功' });
    expect(sonner.toast.success).toHaveBeenCalledWith('操作成功', expect.any(Object));
    expect(id).toBe('success-id');
  });

  it('calls error toast with action', () => {
    const action = { label: '重试', onClick: vi.fn() };
    showToast({ type: 'error', message: '操作失败', action });
    expect(sonner.toast.error).toHaveBeenCalledWith(
      '操作失败',
      expect.objectContaining({ action })
    );
  });

  it('calls loading toast with infinite duration', () => {
    showToast({ type: 'loading', message: '正在处理...', duration: Infinity });
    expect(sonner.toast.loading).toHaveBeenCalledWith(
      '正在处理...',
      expect.objectContaining({ duration: Infinity })
    );
  });

  it('calls info toast with description', () => {
    showToast({ type: 'info', message: '提示', description: '详细信息' });
    expect(sonner.toast.info).toHaveBeenCalledWith(
      '提示',
      expect.objectContaining({ description: '详细信息' })
    );
  });
});

describe('dismissToast', () => {
  it('dismisses toast by id', () => {
    dismissToast('test-id');
    expect(sonner.toast.dismiss).toHaveBeenCalledWith('test-id');
  });
});

describe('promiseToast', () => {
  it('wraps promise with toast messages', async () => {
    const promise = Promise.resolve('result');
    const messages = {
      loading: '加载中...',
      success: '加载成功',
      error: '加载失败',
    };
    const result = await promiseToast(promise, messages);
    expect(sonner.toast.promise).toHaveBeenCalledWith(promise, messages);
    expect(result).toBe('result');
  });
});