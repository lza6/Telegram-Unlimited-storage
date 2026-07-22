import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ShortcutsHelp } from './ShortcutsHelp';

describe('ShortcutsHelp', () => {
  it('renders when open', () => {
    render(<ShortcutsHelp open={true} onClose={vi.fn()} />);
    expect(screen.getByText('键盘快捷键')).toBeInTheDocument();
    expect(screen.getByText('显示/隐藏快捷键帮助')).toBeInTheDocument();
  });

  it('does not render when closed', () => {
    const { container } = render(<ShortcutsHelp open={false} onClose={vi.fn()} />);
    expect(container.firstChild).toBeNull();
  });

  it('calls onClose when close button clicked', () => {
    const onClose = vi.fn();
    render(<ShortcutsHelp open={true} onClose={onClose} />);
    fireEvent.click(screen.getByLabelText('关闭'));
    expect(onClose).toHaveBeenCalled();
  });

  it('calls onClose when overlay clicked', () => {
    const onClose = vi.fn();
    render(<ShortcutsHelp open={true} onClose={onClose} />);
    // Click the overlay (the fixed inset-0 div)
    const overlay = document.querySelector('.fixed.inset-0') as HTMLElement;
    fireEvent.click(overlay);
    expect(onClose).toHaveBeenCalled();
  });

  it('calls onClose when Escape pressed', () => {
    const onClose = vi.fn();
    render(<ShortcutsHelp open={true} onClose={onClose} />);
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('renders all shortcuts', () => {
    render(<ShortcutsHelp open={true} onClose={vi.fn()} />);
    expect(screen.getByText('全选文件')).toBeInTheDocument();
    expect(screen.getByText('删除选中文件')).toBeInTheDocument();
    expect(screen.getByText('预览选中文件')).toBeInTheDocument();
  });

  it('renders keyboard keys in kbd elements', () => {
    render(<ShortcutsHelp open={true} onClose={vi.fn()} />);
    const kbds = screen.getAllByText('?', { exact: false });
    expect(kbds.length).toBeGreaterThan(0);
  });
});