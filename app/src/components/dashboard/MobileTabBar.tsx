import { HardDrive, Search, Settings } from 'lucide-react';

interface MobileTabBarProps {
  activeTab: 'files' | 'search' | 'settings';
  onTabChange: (tab: 'files' | 'search' | 'settings') => void;
  onOpenSidebar: () => void;
}

export function MobileTabBar({ activeTab, onTabChange, onOpenSidebar }: MobileTabBarProps) {
  return (
    <nav
      className="fixed bottom-0 left-0 right-0 md:hidden bg-telegram-surface border-t border-telegram-border z-[100] safe-area-inset-bottom"
      role="navigation"
      aria-label="Mobile navigation"
    >
      <div className="flex items-center justify-around h-16">
        <button
          onClick={() => {
            onOpenSidebar();
            onTabChange('files');
          }}
          className={`flex flex-col items-center justify-center flex-1 h-full transition-colors ${
            activeTab === 'files'
              ? 'text-telegram-primary'
              : 'text-telegram-subtext hover:text-telegram-text'
          }`}
          aria-label="Files"
          aria-current={activeTab === 'files' ? 'page' : undefined}
        >
          <HardDrive className="w-6 h-6" />
          <span className="text-xs mt-1">文件</span>
        </button>

        <button
          onClick={() => onTabChange('search')}
          className={`flex flex-col items-center justify-center flex-1 h-full transition-colors ${
            activeTab === 'search'
              ? 'text-telegram-primary'
              : 'text-telegram-subtext hover:text-telegram-text'
          }`}
          aria-label="Search"
          aria-current={activeTab === 'search' ? 'page' : undefined}
        >
          <Search className="w-6 h-6" />
          <span className="text-xs mt-1">搜索</span>
        </button>

        <button
          onClick={() => onTabChange('settings')}
          className={`flex flex-col items-center justify-center flex-1 h-full transition-colors ${
            activeTab === 'settings'
              ? 'text-telegram-primary'
              : 'text-telegram-subtext hover:text-telegram-text'
          }`}
          aria-label="Settings"
          aria-current={activeTab === 'settings' ? 'page' : undefined}
        >
          <Settings className="w-6 h-6" />
          <span className="text-xs mt-1">设置</span>
        </button>
      </div>
    </nav>
  );
}
