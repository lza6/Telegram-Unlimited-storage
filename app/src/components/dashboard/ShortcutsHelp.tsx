import { useEffect, useRef } from 'react';
import { X } from 'lucide-react';

export interface ShortcutItem {
  key: string;
  description: string;
}

const SHORTCUTS: ShortcutItem[] = [
  { key: '?', description: '显示/隐藏快捷键帮助' },
  { key: 'Ctrl/Cmd + A', description: '全选文件' },
  { key: 'Delete', description: '删除选中文件' },
  { key: 'Esc', description: '关闭弹窗/取消选择' },
  { key: 'Space', description: '预览选中文件' },
  { key: 'N', description: '新建文件夹' },
  { key: 'R', description: '刷新文件列表' },
  { key: 'Enter', description: '打开选中文件夹' },
];

interface ShortcutsHelpProps {
  open: boolean;
  onClose: () => void;
}

export function ShortcutsHelp({ open, onClose }: ShortcutsHelpProps) {
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', handler);
    // Focus panel for accessibility
    panelRef.current?.focus();
    return () => document.removeEventListener('keydown', handler);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-label="快捷键帮助"
    >
      <div
        ref={panelRef}
        className="bg-[#1a2332] rounded-xl border border-[#2d3a4f] p-6 max-w-md w-full mx-4 shadow-xl"
        onClick={(e) => e.stopPropagation()}
        tabIndex={-1}
      >
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-lg font-semibold text-[#e7ecf3]">键盘快捷键</h3>
          <button
            onClick={onClose}
            className="p-1 hover:bg-[#243044] rounded transition-colors text-[#8b9cb3] hover:text-[#e7ecf3]"
            aria-label="关闭"
          >
            <X size={18} />
          </button>
        </div>
        <dl className="space-y-2">
          {SHORTCUTS.map((item) => (
            <div key={item.key} className="flex items-center justify-between py-1">
              <dd className="text-sm text-[#8b9cb3]">{item.description}</dd>
              <dt>
                <kbd className="px-2 py-1 bg-[#0f1419] rounded text-xs font-mono text-[#e7ecf3] border border-[#2d3a4f]">
                  {item.key}
                </kbd>
              </dt>
            </div>
          ))}
        </dl>
        <p className="mt-4 text-xs text-[#8b9cb3] text-center">按 Esc 关闭此面板</p>
      </div>
    </div>
  );
}
