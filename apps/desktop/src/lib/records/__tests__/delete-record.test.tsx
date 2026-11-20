import type { ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vite-plus/test';

import { recordsOptions, useDeleteRecord } from '../records';
import type { RecordListItem } from '../records.type';

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

function listItem(id: string): RecordListItem {
  return {
    id,
    name: `Record ${id}`,
    hasActiveSource: false,
  };
}

describe('delete record', () => {
  it('drops the deleted record from the record list', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'list_records') return Promise.resolve([listItem('b')]);
      return Promise.resolve(undefined);
    });
    const queryClient = new QueryClient();
    queryClient.setQueryData(recordsOptions.queryKey, [listItem('a'), listItem('b')]);

    function wrapper({ children }: { children: ReactNode }) {
      return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
    }

    const { result } = renderHook(() => useDeleteRecord(), { wrapper });
    await result.current.mutateAsync({ recordId: 'a' });

    expect(invoke).toHaveBeenCalledWith('delete_record', { recordId: 'a' });
    await waitFor(() => {
      const records = queryClient.getQueryData<RecordListItem[]>(recordsOptions.queryKey);
      expect(records?.map((record) => record.id)).toEqual(['b']);
    });
  });
});
