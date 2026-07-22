import { fireEvent, render, screen } from '@testing-library/react';
import { Folder } from 'lucide-react';
import { describe, expect, it, vi } from 'vitest';
import { SidebarItem } from './SidebarItem';

describe('SidebarItem accessibility', () => {
    it('provides distinct keyboard-accessible navigation and delete controls', () => {
        const onClick = vi.fn();
        const onDelete = vi.fn();
        render(
            <SidebarItem
                icon={Folder}
                label="Projects"
                active={true}
                folderId={5}
                onClick={onClick}
                onDrop={vi.fn()}
                onDelete={onDelete}
            />,
        );

        const item = screen.getByRole('button', { name: 'Projects' });
        expect(item).toHaveAttribute('aria-current', 'page');
        item.focus();
        fireEvent.click(item);
        expect(onClick).toHaveBeenCalledTimes(1);

        const remove = screen.getByRole('button', { name: 'Delete folder Projects' });
        fireEvent.click(remove);
        expect(onDelete).toHaveBeenCalledTimes(1);
    });
});
