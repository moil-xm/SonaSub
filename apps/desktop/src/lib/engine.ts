import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { queryOptions, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import type { EngineInfo } from '@sonasub/protocol/engine';

export type EngineState =
  | { state: 'stopped' }
  | { state: 'starting'; pid: number | null }
  | { state: 'running'; info: EngineInfo; port: number }
  | { state: 'stopping'; info: EngineInfo; port: number }
  | { state: 'failed'; error: string };

export const engineStatusOptions = queryOptions({
  queryKey: ['engine', 'status'],
  queryFn: () => invoke<EngineState>('status_command'),
  networkMode: 'always',
  retry: false,
  refetchOnWindowFocus: true,
});

export function useEngineStatus() {
  return useQuery(engineStatusOptions);
}

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

export function useEngineAction(action: 'start' | 'stop' | 'restart') {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: ['engine', action],
    networkMode: 'always',
    mutationFn: () => invoke<EngineState>(`${action}_command`),
    onSuccess: (state) => {
      queryClient.setQueryData(engineStatusOptions.queryKey, state);
    },
    onSettled: () => {
      void queryClient.invalidateQueries({ queryKey: engineStatusOptions.queryKey });
    },
  });
}
