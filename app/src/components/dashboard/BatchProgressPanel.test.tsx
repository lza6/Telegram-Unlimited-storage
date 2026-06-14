import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { BatchProgressPanel, TaskItem } from './BatchProgressPanel';

const mockTasks: TaskItem[] = [
  { id: '1', name: 'file1.txt', status: 'running', percent: 50, speed: '1MB/s', remaining: '10秒' },
  { id: '2', name: 'file2.txt', status: 'completed', percent: 100 },
  { id: '3', name: 'file3.txt', status: 'failed', percent: 30 },
];

describe('BatchProgressPanel', () => {
  it('renders task summary', () => {
    render(<BatchProgressPanel tasks={mockTasks} onCancel={vi.fn()} onCancelAll={vi.fn()} />);
    expect(screen.getByText('运行 1 · 完成 1 · 失败 1')).toBeInTheDocument();
  });

  it('renders task names', () => {
    render(<BatchProgressPanel tasks={mockTasks} onCancel={vi.fn()} onCancelAll={vi.fn()} />);
    expect(screen.getByText('file1.txt')).toBeInTheDocument();
    expect(screen.getByText('file2.txt')).toBeInTheDocument();
    expect(screen.getByText('file3.txt')).toBeInTheDocument();
  });

  it('calls onCancel when cancel button clicked', () => {
    const onCancel = vi.fn();
    render(<BatchProgressPanel tasks={mockTasks} onCancel={onCancel} onCancelAll={vi.fn()} />);
    // Find cancel buttons for individual files (not "全部取消")
    const fileCancelButton = screen.getByLabelText('取消 file1.txt');
    fireEvent.click(fileCancelButton);
    expect(onCancel).toHaveBeenCalledWith('1');
  });

  it('calls onCancelAll when close button clicked', () => {
    const onCancelAll = vi.fn();
    render(<BatchProgressPanel tasks={mockTasks} onCancel={vi.fn()} onCancelAll={onCancelAll} />);
    fireEvent.click(screen.getByLabelText('全部取消'));
    expect(onCancelAll).toHaveBeenCalled();
  });

  it('toggles collapse when collapse button clicked', () => {
    render(<BatchProgressPanel tasks={mockTasks} onCancel={vi.fn()} onCancelAll={vi.fn()} />);
    const collapseButton = screen.getByLabelText('收起');
    fireEvent.click(collapseButton);
    expect(screen.getByLabelText('展开')).toBeInTheDocument();
    expect(screen.queryByText('file1.txt')).not.toBeInTheDocument();
  });

  it('returns null when no tasks', () => {
    const { container } = render(
      <BatchProgressPanel tasks={[]} onCancel={vi.fn()} onCancelAll={vi.fn()} />
    );
    expect(container.firstChild).toBeNull();
  });

  it('shows status text for each task', () => {
    render(<BatchProgressPanel tasks={mockTasks} onCancel={vi.fn()} onCancelAll={vi.fn()} />);
    expect(screen.getByText('1MB/s · 10秒')).toBeInTheDocument();
    expect(screen.getByText('完成')).toBeInTheDocument();
    expect(screen.getByText('失败')).toBeInTheDocument();
  });

  it('hides cancel button for completed tasks', () => {
    render(<BatchProgressPanel tasks={mockTasks} onCancel={vi.fn()} onCancelAll={vi.fn()} />);
    const cancelButtons = screen.getAllByLabelText(/取消 file/);
    // Should have 2 cancel buttons (running and failed, not completed)
    // Note: "全部取消" is also a cancel button, so we filter by file name pattern
    expect(cancelButtons.length).toBe(2);
  });
});