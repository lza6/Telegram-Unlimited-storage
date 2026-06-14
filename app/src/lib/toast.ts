import { toast } from 'sonner';

export type ToastType = 'success' | 'error' | 'loading' | 'info';

export interface ShowToastOptions {
  type: ToastType;
  message: string;
  description?: string;
  duration?: number;
  action?: {
    label: string;
    onClick: () => void;
  };
}

export function showToast(options: ShowToastOptions): string {
  const { type, message, description, duration, action } = options;
  const common = {
    description,
    duration,
    action: action ? { label: action.label, onClick: action.onClick } : undefined,
  };

  switch (type) {
    case 'success':
      return toast.success(message, common) as string;
    case 'error':
      return toast.error(message, common) as string;
    case 'loading':
      return toast.loading(message, common) as string;
    case 'info':
    default:
      return toast.info(message, common) as string;
  }
}

export function dismissToast(id: string): void {
  toast.dismiss(id);
}

export async function promiseToast<T>(
  promise: Promise<T>,
  messages: {
    loading: string;
    success: string;
    error: string;
  }
): Promise<T> {
  return toast.promise(promise, messages) as unknown as Promise<T>;
}
