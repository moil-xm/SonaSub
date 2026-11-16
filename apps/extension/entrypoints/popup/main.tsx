import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { I18nextProvider } from '@sonasub/i18n';

import { initializeI18n } from './i18n';
import { Popup } from './popup';
import './popup.css';

const i18n = await initializeI18n();

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <I18nextProvider i18n={i18n}>
      <Popup />
    </I18nextProvider>
  </StrictMode>,
);
