import { useState } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ConfirmProvider, useConfirm } from './ConfirmContext';

function Trigger() {
    const { confirm } = useConfirm();
    const [result, setResult] = useState<string>('pending');

    return (
        <>
            <button onClick={() => confirm({ title: 'Delete file', message: 'This action cannot be undone.', variant: 'danger' }).then(value => setResult(String(value)))}>
                Delete file
            </button>
            <output>{result}</output>
        </>
    );
}

describe('ConfirmProvider', () => {
    it('moves focus into a danger dialog, cancels on Escape, and restores the trigger focus', async () => {
        render(<ConfirmProvider><Trigger /></ConfirmProvider>);

        const trigger = screen.getByRole('button', { name: 'Delete file' });
        trigger.focus();
        fireEvent.click(trigger);

        const dialog = screen.getByRole('alertdialog', { name: 'Delete file' });
        const cancel = screen.getByRole('button', { name: 'Cancel' });
        expect(document.activeElement).toBe(cancel);

        fireEvent.keyDown(dialog, { key: 'Escape' });

        await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('false'));
        await waitFor(() => expect(document.activeElement).toBe(trigger));
    });

    it('settles only once when the confirm control is activated repeatedly', async () => {
        render(<ConfirmProvider><Trigger /></ConfirmProvider>);
        fireEvent.click(screen.getByRole('button', { name: 'Delete file' }));

        const confirm = screen.getByRole('button', { name: 'Confirm' });
        fireEvent.click(confirm);
        fireEvent.click(confirm);

        await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('true'));
    });
});
