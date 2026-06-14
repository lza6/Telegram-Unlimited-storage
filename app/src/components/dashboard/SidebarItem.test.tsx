import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/react';
import { Folder } from 'lucide-react';
import { SidebarItem } from './SidebarItem';

describe('SidebarItem', () => {
    it('does not invoke onDrop when dropEnabled is false', () => {
        const onDrop = vi.fn();
        const { getByRole } = render(
            <SidebarItem
                icon={Folder}
                label="Test"
                active={false}
                onClick={vi.fn()}
                onDrop={onDrop}
                folderId={1}
                dropEnabled={false}
            />,
        );
        const btn = getByRole('button');
        fireEvent.drop(btn, { dataTransfer: { getData: () => '' } });
        expect(onDrop).not.toHaveBeenCalled();
    });

    it('invokes onDrop when dropEnabled is true', () => {
        const onDrop = vi.fn();
        const { getByRole } = render(
            <SidebarItem
                icon={Folder}
                label="Test"
                active={false}
                onClick={vi.fn()}
                onDrop={onDrop}
                folderId={1}
                dropEnabled={true}
            />,
        );
        const btn = getByRole('button');
        fireEvent.drop(btn, { dataTransfer: { getData: () => '' } });
        expect(onDrop).toHaveBeenCalledTimes(1);
    });
});
