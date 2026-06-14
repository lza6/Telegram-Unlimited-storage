import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MobileTabBar } from './MobileTabBar';

describe('MobileTabBar', () => {
  const defaultProps = {
    activeTab: 'files' as const,
    onTabChange: vi.fn(),
    onOpenSidebar: vi.fn(),
  };

  it('renders three tabs', () => {
    render(<MobileTabBar {...defaultProps} />);
    expect(screen.getByLabelText('Files')).toBeInTheDocument();
    expect(screen.getByLabelText('Search')).toBeInTheDocument();
    expect(screen.getByLabelText('Settings')).toBeInTheDocument();
  });

  it('highlights active tab', () => {
    render(<MobileTabBar {...defaultProps} activeTab="search" />);
    expect(screen.getByLabelText('Search')).toHaveAttribute('aria-current', 'page');
    expect(screen.getByLabelText('Files')).not.toHaveAttribute('aria-current', 'page');
  });

  it('calls onTabChange when tab clicked', () => {
    const onTabChange = vi.fn();
    render(<MobileTabBar {...defaultProps} onTabChange={onTabChange} />);

    fireEvent.click(screen.getByLabelText('Search'));
    expect(onTabChange).toHaveBeenCalledWith('search');
  });

  it('calls onOpenSidebar when files tab clicked', () => {
    const onOpenSidebar = vi.fn();
    render(<MobileTabBar {...defaultProps} onOpenSidebar={onOpenSidebar} />);

    fireEvent.click(screen.getByLabelText('Files'));
    expect(onOpenSidebar).toHaveBeenCalled();
  });
});
