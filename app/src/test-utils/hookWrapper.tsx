import { ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SettingsProvider } from '../context/SettingsContext';

export function createHookWrapper() {
    const queryClient = new QueryClient({
        defaultOptions: {
            queries: { retry: false },
            mutations: { retry: false },
        },
    });

    return function HookWrapper({ children }: { children: ReactNode }) {
        return (
            <QueryClientProvider client={queryClient}>
                <SettingsProvider>{children}</SettingsProvider>
            </QueryClientProvider>
        );
    };
}
