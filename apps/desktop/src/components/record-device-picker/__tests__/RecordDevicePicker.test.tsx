import { createI18n, I18nextProvider } from '@sonasub/i18n';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import { recordDevicesOptions } from '@/lib/records';
import type { RecordDevice } from '@/lib/records';

import { RecordDevicePicker } from '../RecordDevicePicker';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

const devices: RecordDevice[] = [
  {
    kind: 'microphone',
    id: 'coreaudio:mic',
    name: 'USB Headset',
    isDefault: false,
    captureSupported: true,
    captureReason: null,
    isAvailable: false,
    occupiedRecordId: 'record-b',
    occupiedRecordName: 'Record B',
  },
];

afterEach(() => {
  cleanup();
  invoke.mockReset();
});

describe('record device picker', () => {
  it('refetches devices from the refresh button', async () => {
    invoke.mockResolvedValue(devices);
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    await queryClient.prefetchQuery(recordDevicesOptions);
    invoke.mockClear();

    render(
      <I18nextProvider i18n={createI18n('en-US')}>
        <QueryClientProvider client={queryClient}>
          <RecordDevicePicker
            devices={devices}
            selected={[]}
            disabled={false}
            onSelect={() => undefined}
            onRemove={() => undefined}
          />
        </QueryClientProvider>
      </I18nextProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Add device' }));
    const refresh = await screen.findByRole('button', { name: 'Refresh device list' });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('list_record_devices');
    });

    invoke.mockClear();
    fireEvent.click(refresh);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('list_record_devices');
    });
  });

  it('shows an occupied selected device in red on the right side of its name', () => {
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });

    render(
      <I18nextProvider i18n={createI18n('en-US')}>
        <QueryClientProvider client={queryClient}>
          <RecordDevicePicker
            devices={devices}
            selected={[{ kind: 'microphone', id: 'coreaudio:mic' }]}
            currentRecordId="record-a"
            disabled
            onSelect={() => undefined}
            onRemove={() => undefined}
          />
        </QueryClientProvider>
      </I18nextProvider>,
    );

    const name = screen.getByText('USB Headset');
    const chip = name.parentElement;
    if (!chip) throw new Error('Device chip was not rendered.');
    const status = screen.getByRole('img', { name: 'In use by Record B' });
    const children = Array.from(chip.children);

    expect(status.className).toContain('bg-destructive');
    expect(children.indexOf(status)).toBeGreaterThan(children.indexOf(name));
  });

  it('shows an occupied device in red on the right side of the list row', async () => {
    invoke.mockResolvedValue(devices);
    const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    await queryClient.prefetchQuery(recordDevicesOptions);

    render(
      <I18nextProvider i18n={createI18n('en-US')}>
        <QueryClientProvider client={queryClient}>
          <RecordDevicePicker
            devices={devices}
            selected={[]}
            disabled={false}
            onSelect={() => undefined}
            onRemove={() => undefined}
          />
        </QueryClientProvider>
      </I18nextProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Add device' }));
    const name = await screen.findByText('USB Headset');
    const row = name.closest('button');
    if (!row) throw new Error('Device row was not rendered.');
    const status = screen.getByRole('img', { name: 'In use by Record B' });

    expect(status.className).toContain('bg-destructive');
    expect(row.lastElementChild).toBe(status);
  });
});
