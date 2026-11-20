import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import {
  createI18n,
  getLocaleForPreference,
  I18nextProvider,
  isLanguagePreference,
} from '@sonasub/i18n';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

import { FloatingCaptionCard } from './components/floating-caption-card';
import './styles/app.css';

const LANGUAGE_STORAGE_KEY = 'sonasub-language';
const storedPreference: unknown = localStorage.getItem(LANGUAGE_STORAGE_KEY);
const preference = isLanguagePreference(storedPreference) ? storedPreference : 'system';
const overlayI18n = createI18n(getLocaleForPreference(preference));

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

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <I18nextProvider i18n={overlayI18n}>
      <QueryClientProvider client={queryClient}>
        <FloatingCaptionCard />
      </QueryClientProvider>
    </I18nextProvider>
  </StrictMode>,
);
