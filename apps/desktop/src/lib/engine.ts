import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import type { EngineAction, EngineState } from './engine.type';

export type { EngineAction, EngineState } from './engine.type';

export const engineStatusOptions = queryOptions({
  queryKey: ['engine', 'status'],
  queryFn: () => invoke<EngineState>('status_command'),
  networkMode: 'always',
  retry: false,
  refetchOnWindowFocus: true,
});

export function useEngineEvents() {
  const queryClient = useQueryClient();
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let disposed = false;
    const subscription = listen<EngineState>('engine://state', (event) => {
      queryClient.setQueryData(engineStatusOptions.queryKey, event.payload);
      void queryClient.invalidateQueries({ queryKey: engineStatusOptions.queryKey });
    });
    void subscription.then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [queryClient]);
}

export function useEngineAction() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: ['engine', 'action'],
    scope: { id: 'engine-action' },
    networkMode: 'always',
    mutationFn: (action: EngineAction) => invoke<EngineState>(`${action}_command`),
    onSuccess: (state) => {
      queryClient.setQueryData(engineStatusOptions.queryKey, state);
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: engineStatusOptions.queryKey });
    },
  });
}
