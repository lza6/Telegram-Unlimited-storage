import { useEffect, useCallback } from 'react';

interface UseKeyboardShortcutsProps {
    onSelectAll: () => void;
    onDelete: () => void;
    onEscape: () => void;
    onSearch: () => void;
    onEnter?: () => void;
    onToggleHelp?: () => void;
    /** Navigation shortcuts (Escape, search) */
    enabled?: boolean;
    /** Select-all shortcut */
    transferEnabled?: boolean;
    /** Delete shortcut — Bot index delete when User session offline */
    deleteEnabled?: boolean;
    /** Enter preview shortcut — mirrors previewReady */
    previewEnabled?: boolean;
}

export function useKeyboardShortcuts({
    onSelectAll,
    onDelete,
    onEscape,
    onSearch,
    onEnter,
    onToggleHelp,
    enabled = true,
    transferEnabled = true,
    deleteEnabled = transferEnabled,
    previewEnabled = transferEnabled,
}: UseKeyboardShortcutsProps) {

    const handleKeyDown = useCallback((e: KeyboardEvent) => {
        if (!enabled) return;

        // Don't trigger shortcuts when typing in inputs
        const target = e.target as HTMLElement;
        if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
            // Only allow Escape in inputs
            if (e.key === 'Escape') {
                (target as HTMLInputElement).blur();
                onEscape();
            }
            return;
        }

        const isMod = e.metaKey || e.ctrlKey;

        // Cmd/Ctrl + A - Select All
        if (isMod && e.key === 'a') {
            if (!transferEnabled) return;
            e.preventDefault();
            onSelectAll();
            return;
        }

        // Cmd/Ctrl + F - Focus Search
        if (isMod && e.key === 'f') {
            e.preventDefault();
            onSearch();
            return;
        }

        // Delete / Backspace - Delete selected
        if (e.key === 'Delete' || e.key === 'Backspace') {
            if (!deleteEnabled) return;
            e.preventDefault();
            onDelete();
            return;
        }

        // Escape - Clear selection
        if (e.key === 'Escape') {
            e.preventDefault();
            onEscape();
            return;
        }
        // Enter - Open / Preview
        if (e.key === 'Enter') {
            if (!previewEnabled) return;
            e.preventDefault();
            onEnter?.();
            return;
        }

        // ? - Toggle help
        if (e.key === '?') {
            e.preventDefault();
            onToggleHelp?.();
            return;
        }
    }, [enabled, transferEnabled, deleteEnabled, previewEnabled, onSelectAll, onDelete, onEscape, onSearch, onEnter, onToggleHelp]);

    useEffect(() => {
        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [handleKeyDown]);
}
