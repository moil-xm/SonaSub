import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query';

import {
  appendSessionNotice,
  applySessionStats,
  applySessionStatus,
  applySubtitleBatch,
} from '../session-cache';
import type {
  AppConfig,
  Capabilities,
  ConfigView,
  SessionAction,
  SessionNotice,
  SessionSnapshot,
  SessionStats,
  SessionStatus,
  SubtitleBatch,
} from './session.type';

export const sessionCapabilitiesOptions = queryOptions({
  queryKey: ['session', 'capabilities'],
  queryFn: () => invoke<Capabilities>('get_capabilities'),
  networkMode: 'always',
  retry: false,
  staleTime: 15_000,
  refetchOnWindowFocus: true,
});

export const sessionConfigOptions = queryOptions({
  queryKey: ['session', 'config'],
  queryFn: () => invoke<ConfigView>('get_config'),
  networkMode: 'always',
  retry: false,
  refetchOnWindowFocus: true,
});

export const sessionSnapshotOptions = queryOptions({
  queryKey: ['session', 'snapshot'],
  queryFn: () => invoke<SessionSnapshot>('session_snapshot'),
  networkMode: 'always',
  retry: false,
  refetchOnWindowFocus: true,
});

export function useSessionEvents() {
  const queryClient = useQueryClient();

  useEffect(() => {
    let isDisposed = false;
    const unlisteners: UnlistenFn[] = [];
    const snapshotKey = sessionSnapshotOptions.queryKey;

    function track(unlisten: UnlistenFn) {
      if (isDisposed) unlisten();
      else unlisteners.push(unlisten);
    }

    void listen<SessionStatus>('session://status', ({ payload }) => {
      queryClient.setQueryData<SessionSnapshot>(snapshotKey, (snapshot) => {
        if (!snapshot) return undefined;
        return applySessionStatus(snapshot, payload);
      });
    }).then(track);

    void listen<SubtitleBatch>('session://subtitles', ({ payload }) => {
      queryClient.setQueryData<SessionSnapshot>(snapshotKey, (snapshot) => {
        if (!snapshot) return undefined;
        return applySubtitleBatch(snapshot, payload);
      });
    }).then(track);

    void listen<SessionStats>('session://stats', ({ payload }) => {
      queryClient.setQueryData<SessionSnapshot>(snapshotKey, (snapshot) => {
        if (!snapshot) return undefined;
        return applySessionStats(snapshot, payload);
      });
    }).then(track);

    void listen<SessionNotice>('session://notice', ({ payload }) => {
      queryClient.setQueryData<SessionSnapshot>(snapshotKey, (snapshot) => {
        if (!snapshot) return undefined;
        return appendSessionNotice(snapshot, payload);
      });
    }).then(track);

    return () => {
      isDisposed = true;
      for (const unlisten of unlisteners) unlisten();
    };
  }, [queryClient]);
}

export function useSessionAction() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationKey: ['session', 'action'],
    scope: { id: 'session-action' },
    networkMode: 'always',
    mutationFn: (action: SessionAction) => invoke<SessionSnapshot>(`${action}_session`),
    onSuccess: (snapshot) => {
      queryClient.setQueryData(sessionSnapshotOptions.queryKey, snapshot);
    },
  });
}

export function useSaveConfig() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationKey: ['session', 'config', 'save'],
    networkMode: 'always',
    mutationFn: (config: AppConfig) => invoke<ConfigView>('save_config', { config }),
    onSuccess: (view) => {
      queryClient.setQueryData(sessionConfigOptions.queryKey, view);
    },
  });
}
