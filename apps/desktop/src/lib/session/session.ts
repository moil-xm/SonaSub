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
  isSessionSnapshot,
  mergeSnapshot,
} from '../session-cache';
import type {
  AppConfig,
  Capabilities,
  ConfigView,
  SessionAction,
  SessionNotice,
  SessionSnapshot,
  StatsEvent,
  StatusEvent,
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
  // A snapshot that was taken before a newer event must not roll that event
  // back. Every channel carries its own revision, so the merge keeps the
  // newer value per channel.
  structuralSharing: (previous, incoming) =>
    isSessionSnapshot(incoming)
      ? mergeSnapshot(isSessionSnapshot(previous) ? previous : undefined, incoming)
      : incoming,
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

    void listen<StatusEvent>('session://status', ({ payload }) => {
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

    void listen<StatsEvent>('session://stats', ({ payload }) => {
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

    // Events published between the first snapshot read and the subscription
    // are picked up by this read. The revision rules decide which of the two
    // sources is newer.
    void queryClient.invalidateQueries({ queryKey: snapshotKey });

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
      queryClient.setQueryData<SessionSnapshot>(sessionSnapshotOptions.queryKey, (current) =>
        mergeSnapshot(current, snapshot),
      );
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
      // The history limit is part of the session contract, so the cached
      // snapshot follows the saved configuration.
      queryClient.setQueryData<SessionSnapshot>(sessionSnapshotOptions.queryKey, (snapshot) => {
        if (!snapshot) return snapshot;
        return { ...snapshot, historyLimit: view.config.ui.historyLimit };
      });
    },
  });
}
