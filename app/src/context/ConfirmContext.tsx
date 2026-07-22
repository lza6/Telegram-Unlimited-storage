import {
    createContext,
    KeyboardEvent,
    ReactNode,
    useCallback,
    useContext,
    useEffect,
    useRef,
    useState,
} from 'react';

interface ConfirmOptions {
    title: string;
    message: string;
    confirmText?: string;
    cancelText?: string;
    variant?: 'danger' | 'info';
}

interface ConfirmContextType {
    confirm: (options: ConfirmOptions) => Promise<boolean>;
}

const ConfirmContext = createContext<ConfirmContextType | undefined>(undefined);

const FOCUSABLE_SELECTOR = [
    'button:not([disabled])',
    '[href]',
    'input:not([disabled])',
    'select:not([disabled])',
    'textarea:not([disabled])',
    '[tabindex]:not([tabindex="-1"])',
].join(',');

export function ConfirmProvider({ children }: { children: ReactNode }) {
    const [isOpen, setIsOpen] = useState(false);
    const [options, setOptions] = useState<ConfirmOptions>({ title: '', message: '' });
    const resolveRef = useRef<((value: boolean) => void) | null>(null);
    const dialogRef = useRef<HTMLDivElement | null>(null);
    const cancelRef = useRef<HTMLButtonElement | null>(null);
    const returnFocusRef = useRef<HTMLElement | null>(null);

    const restoreFocus = useCallback(() => {
        const trigger = returnFocusRef.current;
        returnFocusRef.current = null;
        if (trigger?.isConnected) {
            requestAnimationFrame(() => trigger.focus());
        }
    }, []);

    const settle = useCallback((value: boolean) => {
        const resolve = resolveRef.current;
        if (!resolve) return;

        resolveRef.current = null;
        setIsOpen(false);
        resolve(value);
        restoreFocus();
    }, [restoreFocus]);

    const confirm = useCallback((opts: ConfirmOptions) => {
        // A caller should never be left waiting if another confirmation replaces it.
        resolveRef.current?.(false);
        returnFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        setOptions(opts);
        setIsOpen(true);
        return new Promise<boolean>((resolve) => {
            resolveRef.current = resolve;
        });
    }, []);

    useEffect(() => {
        if (!isOpen) return;
        // Cancel is the safe default for destructive confirmations.
        cancelRef.current?.focus();
    }, [isOpen]);

    useEffect(() => () => {
        resolveRef.current?.(false);
        resolveRef.current = null;
    }, []);

    const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
        if (event.key === 'Escape') {
            event.preventDefault();
            settle(false);
            return;
        }
        if (event.key !== 'Tab') return;

        const focusable = Array.from(dialogRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR) ?? []);
        if (!focusable.length) {
            event.preventDefault();
            return;
        }

        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (event.shiftKey && document.activeElement === first) {
            event.preventDefault();
            last.focus();
        } else if (!event.shiftKey && document.activeElement === last) {
            event.preventDefault();
            first.focus();
        }
    };

    return (
        <ConfirmContext.Provider value={{ confirm }}>
            {children}
            {isOpen && (
                <div
                    className="fixed inset-0 z-[200] flex items-center justify-center bg-black/50 p-4 backdrop-blur-sm"
                    onMouseDown={(event) => {
                        if (event.target === event.currentTarget) settle(false);
                    }}
                >
                    <div
                        ref={dialogRef}
                        role={options.variant === 'danger' ? 'alertdialog' : 'dialog'}
                        aria-modal="true"
                        aria-labelledby="confirm-title"
                        aria-describedby="confirm-message"
                        onKeyDown={handleKeyDown}
                        className="w-full max-w-md rounded-xl border border-white/10 bg-[#1c1c1c] p-6 shadow-2xl animate-in zoom-in-95"
                    >
                        <h3 id="confirm-title" className="mb-2 text-lg font-medium text-white">{options.title}</h3>
                        <p id="confirm-message" className="mb-6 whitespace-pre-line text-sm text-telegram-subtext">{options.message}</p>
                        <div className="flex justify-end gap-3">
                            <button
                                ref={cancelRef}
                                type="button"
                                onClick={() => settle(false)}
                                className="rounded-lg px-4 py-2 text-sm font-medium text-telegram-subtext transition hover:bg-white/5"
                            >
                                {options.cancelText || 'Cancel'}
                            </button>
                            <button
                                type="button"
                                onClick={() => settle(true)}
                                className={`rounded-lg px-4 py-2 text-sm font-medium transition ${options.variant === 'danger' ? 'bg-red-500/10 text-red-400 hover:bg-red-500/20' : 'bg-telegram-primary text-white hover:bg-telegram-primary/90'}`}
                            >
                                {options.confirmText || 'Confirm'}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </ConfirmContext.Provider>
    );
}

export const useConfirm = () => {
    const context = useContext(ConfirmContext);
    if (!context) throw new Error('useConfirm must be used within a ConfirmProvider');
    return context;
};
