import { createI18n, I18nextProvider } from '@sonasub/i18n';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import type { EngineState } from '@/lib/engine.type';

import { EngineStatus } from './engine-status';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => vi.fn()) }));

const failedState: EngineState = {
  state: 'failed',
  error: 'Startup connection failed',
};
const runningState: EngineState = {
  state: 'running',
  port: 43123,
  info: {
    engineVersion: '0.0.0',
    nodeVersion: process.version,
    pid: 12345,
    startedAt: '2026-11-16T08:00:00.000Z',
  },
};

function renderEngineStatus(queryClient: QueryClient) {
  const i18n = createI18n('en-US');
  return render(
    <I18nextProvider i18n={i18n}>
      <QueryClientProvider client={queryClient}>
        <EngineStatus />
      </QueryClientProvider>
    </I18nextProvider>,
  );
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('EngineStatus', () => {
  it('clears a failed start when restart recovers the engine', async () => {
    const invokeMock = vi.mocked(invoke);
    let backendState: EngineState = failedState;
    invokeMock.mockImplementation(async (command) => {
      if (command === 'start_command') throw new Error('Start connection failed');
      if (command === 'restart_command') {
        backendState = runningState;
        return runningState;
      }
      return backendState;
    });
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    const user = userEvent.setup();
    renderEngineStatus(queryClient);

    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('Failed'));
    await user.click(screen.getByRole('button', { name: 'Start' }));
    await waitFor(() =>
      expect(
        queryClient.getMutationCache().find({ mutationKey: ['engine', 'action'] })?.state.status,
      ).toBe('error'),
    );

    await user.click(screen.getByRole('button', { name: 'Restart' }));
    await waitFor(() => expect(screen.getByRole('status').textContent).toContain('Running'));
    expect(screen.queryByRole('alert')).toBeNull();
    expect(screen.getByText(process.version).textContent).toBe(process.version);
  });
});
