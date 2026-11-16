import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { I18nextProvider } from '@sonasub/i18n';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider, createRouter } from '@tanstack/react-router';

import { i18n } from './i18n';
import { routeTree } from './routeTree';
import './styles/app.css';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      networkMode: 'always',
      retry: false,
      refetchOnWindowFocus: false,
    },
    mutations: {
      networkMode: 'always',
      retry: false,
    },
  },
});
const router = createRouter({ routeTree });

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </I18nextProvider>
  </StrictMode>,
);
