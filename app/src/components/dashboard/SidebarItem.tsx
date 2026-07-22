import { memo, useState } from 'react';
import { Plus } from 'lucide-react';

interface SidebarItemProps {
    icon: React.ElementType;
    label: string;
    active: boolean;
    onClick: () => void;
    onDrop: (e: React.DragEvent) => void;
    onDelete?: () => void;
    folderId: number | null;
    dropEnabled?: boolean;
}

/**
 * SidebarItem - Pure DOM event-based drop handling.
 * With Tauri's dragDropEnabled: false, DOM events work reliably.
 */
export const SidebarItem = memo(function SidebarItem({ icon: Icon, label, active = false, onClick, onDrop, onDelete, dropEnabled = true }: SidebarItemProps) {
    const [isOver, setIsOver] = useState(false);
    const itemClassName = `group flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm font-medium transition-all duration-150 ${active
        ? 'bg-telegram-primary/10 text-telegram-primary'
        : isOver
            ? 'bg-telegram-primary/30 text-telegram-text ring-2 ring-telegram-primary scale-[1.02] shadow-lg'
            : 'text-telegram-subtext hover:bg-telegram-hover hover:text-telegram-text'
        }`;

    return (
        <div
            className="group relative"
            onDragEnter={(e) => {
                if (!dropEnabled) return;
                e.preventDefault();
                e.stopPropagation();
                setIsOver(true);
            }}
            onDragOver={(e) => {
                if (!dropEnabled) return;
                e.preventDefault();
                e.stopPropagation();
                e.dataTransfer.dropEffect = 'move';
            }}
            onDragLeave={(e) => {
                if (!dropEnabled) return;
                const rect = e.currentTarget.getBoundingClientRect();
                if (e.clientX < rect.left || e.clientX > rect.right || e.clientY < rect.top || e.clientY > rect.bottom) {
                    setIsOver(false);
                }
            }}
            onDrop={(e) => {
                e.preventDefault();
                e.stopPropagation();
                setIsOver(false);
                if (dropEnabled) onDrop(e);
            }}
            onContextMenu={(e) => {
                if (!onDelete) return;
                e.preventDefault();
                onDelete();
            }}
        >
            <button
                type="button"
                onClick={onClick}
                aria-current={active ? 'page' : undefined}
                className={itemClassName}
            >
                <Icon className={`h-4 w-4 ${isOver ? 'text-telegram-primary' : ''}`} aria-hidden="true" />
                <span className="flex-1 truncate text-left">{label}</span>
            </button>
            {onDelete && (
                <button
                    type="button"
                    onClick={onDelete}
                    aria-label={`Delete folder ${label}`}
                    title={`Delete folder ${label}`}
                    className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-1 text-telegram-subtext opacity-0 transition hover:text-red-400 focus-visible:opacity-100 group-hover:opacity-100"
                >
                    <Plus className="h-3 w-3 rotate-45" aria-hidden="true" />
                </button>
            )}
        </div>
    );
});
