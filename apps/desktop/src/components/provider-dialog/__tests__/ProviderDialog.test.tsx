import { createI18n, I18nextProvider } from '@sonasub/i18n';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import { ProviderDialog } from '../ProviderDialog';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

afterEach(() => {
  cleanup();
  invoke.mockReset();
});

describe('ProviderDialog', () => {
  it('shows a manually added model as a selected chip', () => {
    const i18n = createI18n('en-US');
    const queryClient = new QueryClient({
      defaultOptions: { queries: { staleTime: Infinity, retry: false } },
    });

    render(
      <I18nextProvider i18n={i18n}>
        <QueryClientProvider client={queryClient}>
          <ProviderDialog open provider={null} onClose={() => undefined} />
        </QueryClientProvider>
      </I18nextProvider>,
    );

    fireEvent.change(screen.getByPlaceholderText('Add model ID...'), {
      target: { value: 'custom/model' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Add model ID' }));

    expect(screen.getByText('custom/model')).toBeTruthy();
    expect(screen.getByText('1 models')).toBeTruthy();
  });

  it('shows an uploaded icon image and keeps its file name', async () => {
    const i18n = createI18n('en-US');
    const queryClient = new QueryClient({
      defaultOptions: { queries: { staleTime: Infinity, retry: false } },
    });
    const dataUrl =
      'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/n6kAAAAASUVORK5CYII=';
    const file = new File([new Uint8Array([1])], 'provider-icon.png', { type: 'image/png' });
    const loadListeners: Array<() => void> = [];
    const errorListeners: Array<() => void> = [];
    class FileReaderStub {
      result: string | ArrayBuffer | null = null;
      addEventListener(type: string, listener: () => void) {
        if (type === 'load') loadListeners.push(listener);
        if (type === 'error') errorListeners.push(listener);
      }
      readAsDataURL() {
        this.result = dataUrl;
        for (const listener of loadListeners) listener();
      }
    }
    const originalFileReader = globalThis.FileReader;
    Object.defineProperty(globalThis, 'FileReader', {
      configurable: true,
      writable: true,
      value: FileReaderStub,
    });

    try {
      render(
        <I18nextProvider i18n={i18n}>
          <QueryClientProvider client={queryClient}>
            <ProviderDialog open provider={null} onClose={() => undefined} />
          </QueryClientProvider>
        </I18nextProvider>,
      );

      const input = document.querySelector('input[type="file"]');
      if (!(input instanceof HTMLInputElement)) throw new Error('File input was not rendered.');
      fireEvent.change(input, { target: { files: [file] } });

      const image = await screen.findByAltText('provider-icon.png');
      if (!(image instanceof HTMLImageElement)) throw new Error('Icon image was not rendered.');
      expect(image.src).toBe(dataUrl);
      expect(screen.getByText('provider-icon.png')).toBeTruthy();
    } finally {
      Object.defineProperty(globalThis, 'FileReader', {
        configurable: true,
        writable: true,
        value: originalFileReader,
      });
    }
  });

  it('tests only selected models and marks each result', async () => {
    const i18n = createI18n('en-US');
    const queryClient = new QueryClient({
      defaultOptions: { queries: { staleTime: Infinity, retry: false } },
    });
    invoke.mockImplementation((command: string) => {
      if (command === 'fetch_provider_models') {
        return Promise.resolve(['selected/model', 'unselected/model']);
      }
      if (command === 'test_provider_command') {
        return Promise.resolve('OK');
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
    render(
      <I18nextProvider i18n={i18n}>
        <QueryClientProvider client={queryClient}>
          <ProviderDialog
            open
            provider={{
              name: 'OpenAI',
              baseUrl: 'https://api.openai.com/v1',
              apiKey: '',
              icon: { name: '', dataUrl: '' },
              models: ['selected/model'],
              concurrency: 1,
            }}
            onClose={() => undefined}
          />
        </QueryClientProvider>
      </I18nextProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Refresh models' }));
    await screen.findByText('unselected/model');

    fireEvent.click(screen.getByRole('button', { name: 'Test model' }));

    await screen.findByTestId('model-status-selected/model');
    expect(invoke).toHaveBeenCalledWith('fetch_provider_models', {
      baseUrl: 'https://api.openai.com/v1',
      apiKey: '',
    });
    expect(invoke).toHaveBeenCalledWith('test_provider_command', {
      baseUrl: 'https://api.openai.com/v1',
      apiKey: '',
      model: 'selected/model',
    });
    expect(invoke).not.toHaveBeenCalledWith('test_provider_command', {
      baseUrl: 'https://api.openai.com/v1',
      apiKey: '',
      model: 'unselected/model',
    });
    expect(screen.getByTestId('model-status-selected/model').dataset.state).toBe('passed');
    expect(screen.queryByTestId('model-status-unselected/model')).toBeNull();
  });

  it('shows testing progress and failed status for a selected model', async () => {
    const i18n = createI18n('en-US');
    const queryClient = new QueryClient({
      defaultOptions: { queries: { staleTime: Infinity, retry: false } },
    });
    invoke.mockImplementation((command: string) => {
      if (command === 'test_provider_command') {
        return Promise.reject(new Error('API key invalid'));
      }
      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });

    render(
      <I18nextProvider i18n={i18n}>
        <QueryClientProvider client={queryClient}>
          <ProviderDialog
            open
            provider={{
              name: 'OpenAI',
              baseUrl: 'https://api.openai.com/v1',
              apiKey: '',
              icon: { name: '', dataUrl: '' },
              models: ['failing/model'],
              concurrency: 1,
            }}
            onClose={() => undefined}
          />
        </QueryClientProvider>
      </I18nextProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Test model' }));

    await vi.waitFor(() => {
      expect(screen.getByTestId('model-status-failing/model').dataset.state).toBe('failed');
    });
    const testButton = screen.getByRole('button', { name: 'Test model' });
    if (!(testButton instanceof HTMLButtonElement))
      throw new Error('Test button was not rendered.');
    expect(testButton.disabled).toBe(false);
    expect(screen.getByText('Connection failed: API key invalid')).toBeTruthy();
  });
});
