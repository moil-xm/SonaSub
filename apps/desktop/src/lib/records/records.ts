import { useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query';

import { applySubtitlePatches } from '@/lib/session-cache';
import type { AudioDeviceRef, SubtitleSegment } from '@/lib/session';

import type {
  RecordAction,
  RecordDevice,
  RecordInput,
  RecordNoticeEvent,
  RecordSnapshot,
  RecordStatsEvent,
  RecordSubtitleBatch,
  RecordSummary,
  RecordSummaryEvent,
  RecordState,
} from './records.type';

export const recordsOptions = queryOptions({
  queryKey: ['records', 'list'],
  queryFn: () => invoke<RecordSummary[]>('list_records'),
  networkMode: 'always',
  retry: false,
  refetchOnWindowFocus: true,
});

export const recordDevicesOptions = queryOptions({
  queryKey: ['records', 'devices'],
  queryFn: () => invoke<RecordDevice[]>('list_record_devices'),
  networkMode: 'always',
  retry: false,
  staleTime: 2_000,
  refetchOnWindowFocus: true,
});

export function recordSnapshotOptions(recordId: string) {
  return queryOptions({
    queryKey: ['records', recordId, 'snapshot'],
    queryFn: () => invoke<RecordSnapshot>('get_record', { recordId }),
    networkMode: 'always',
    retry: false,
    refetchOnWindowFocus: true,
    structuralSharing: (previous, incoming) => {
      if (!isRecordSnapshot(incoming)) return incoming;
      let current: RecordSnapshot | undefined;
      if (isRecordSnapshot(previous)) current = previous;
      return mergeRecordSnapshot(current, incoming);
    },
  });
}

export function useRecordEvents() {
  const queryClient = useQueryClient();

  useEffect(() => {
    let isDisposed = false;
    const unlisteners: UnlistenFn[] = [];

    function track(unlisten: UnlistenFn) {
      if (isDisposed) unlisten();
      else unlisteners.push(unlisten);
    }

    function applySummary(event: RecordSummaryEvent) {
      queryClient.setQueryData<RecordSummary[]>(recordsOptions.queryKey, (current) => {
        if (!current) return current;
        const next = current.filter((record) => record.id !== event.summary.id);
        next.push(event.summary);
        next.sort((left, right) => {
          if (left.createdAtMs !== right.createdAtMs) return left.createdAtMs - right.createdAtMs;
          return left.id.localeCompare(right.id);
        });
        return next;
      });
      queryClient.setQueryData<RecordSnapshot>(
        recordSnapshotOptions(event.recordId).queryKey,
        (current) => {
          if (!current || event.revision < current.summaryRevision) return current;
          return {
            ...current,
            summary: event.summary,
            summaryRevision: event.revision,
          };
        },
      );
      void queryClient.invalidateQueries({ queryKey: recordsOptions.queryKey });
      void queryClient.invalidateQueries({ queryKey: recordDevicesOptions.queryKey });
    }

    function applySubtitles(batch: RecordSubtitleBatch) {
      queryClient.setQueryData<RecordSnapshot>(
        recordSnapshotOptions(batch.recordId).queryKey,
        (current) => {
          if (!current) return current;
          return applyRecordSubtitleBatch(current, batch);
        },
      );
    }

    function applyStats(event: RecordStatsEvent) {
      queryClient.setQueryData<RecordSnapshot>(
        recordSnapshotOptions(event.recordId).queryKey,
        (current) => {
          if (!current || event.revision <= current.statsRevision) return current;
          return { ...current, stats: event.stats, statsRevision: event.revision };
        },
      );
    }

    function applyNotice(event: RecordNoticeEvent) {
      queryClient.setQueryData<RecordSnapshot>(
        recordSnapshotOptions(event.recordId).queryKey,
        (current) => {
          if (!current) return current;
          return { ...current, notices: [...current.notices, event.notice].slice(-100) };
        },
      );
    }

    void listen<RecordSummaryEvent>('record://summary', ({ payload }) => {
      applySummary(payload);
    }).then(track);
    void listen<RecordSubtitleBatch>('record://subtitles', ({ payload }) => {
      applySubtitles(payload);
    }).then(track);
    void listen<RecordStatsEvent>('record://stats', ({ payload }) => {
      applyStats(payload);
    }).then(track);
    void listen<RecordNoticeEvent>('record://notice', ({ payload }) => {
      applyNotice(payload);
    }).then(track);

    void queryClient.invalidateQueries({ queryKey: recordsOptions.queryKey });
    void queryClient.invalidateQueries({ queryKey: recordDevicesOptions.queryKey });

    return () => {
      isDisposed = true;
      for (const unlisten of unlisteners) unlisten();
    };
  }, [queryClient]);
}

export function useRecordAction() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationKey: ['records', 'action'],
    networkMode: 'always',
    mutationFn: ({ recordId, action }: { recordId: string; action: RecordAction }) =>
      invoke<RecordSnapshot>(`${action}_record`, { recordId }),
    onSuccess: (snapshot) => applySnapshot(queryClient, snapshot),
  });
}

export function useAddRecordDevice() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationKey: ['records', 'device', 'add'],
    networkMode: 'always',
    mutationFn: ({ recordId, device }: { recordId: string; device: AudioDeviceRef }) =>
      invoke<RecordSnapshot>('add_record_device', { recordId, device }),
    onSuccess: (snapshot) => applySnapshot(queryClient, snapshot),
  });
}

export function useRemoveRecordDevice() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationKey: ['records', 'device', 'remove'],
    networkMode: 'always',
    mutationFn: ({ recordId, device }: { recordId: string; device: AudioDeviceRef }) =>
      invoke<RecordSnapshot>('remove_record_device', { recordId, device }),
    onSuccess: (snapshot) => applySnapshot(queryClient, snapshot),
  });
}

export function useCreateRecord() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationKey: ['records', 'create'],
    networkMode: 'always',
    mutationFn: (input: RecordInput) => invoke<RecordSnapshot>('create_record', { input }),
    onSuccess: (snapshot) => applySnapshot(queryClient, snapshot),
  });
}

export function useUpdateRecord() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationKey: ['records', 'update'],
    networkMode: 'always',
    mutationFn: ({ recordId, input }: { recordId: string; input: RecordInput }) =>
      invoke<RecordSnapshot>('update_record', { recordId, input }),
    onSuccess: (snapshot) => applySnapshot(queryClient, snapshot),
  });
}

function applySnapshot(queryClient: ReturnType<typeof useQueryClient>, snapshot: RecordSnapshot) {
  queryClient.setQueryData<RecordSnapshot>(
    recordSnapshotOptions(snapshot.summary.id).queryKey,
    (current) => mergeRecordSnapshot(current, snapshot),
  );
  queryClient.setQueryData<RecordSummary[]>(recordsOptions.queryKey, (current) => {
    if (!current) return current;
    const next = current.filter((record) => record.id !== snapshot.summary.id);
    next.push(snapshot.summary);
    next.sort((left, right) => {
      if (left.createdAtMs !== right.createdAtMs) return left.createdAtMs - right.createdAtMs;
      return left.id.localeCompare(right.id);
    });
    return next;
  });
  void queryClient.invalidateQueries({ queryKey: recordsOptions.queryKey });
  void queryClient.invalidateQueries({ queryKey: recordDevicesOptions.queryKey });
}

export function applyRecordSubtitleBatch(
  snapshot: RecordSnapshot,
  batch: RecordSubtitleBatch,
): RecordSnapshot {
  if (batch.recordId !== snapshot.summary.id || batch.revision <= snapshot.subtitleRevision) {
    return snapshot;
  }
  const existingIds = new Set(snapshot.segments.map((segment) => segment.id));
  let added = 0;
  for (const patch of batch.patches) {
    if (patch.type !== 'upsert' && patch.type !== 'finalize') continue;
    if (existingIds.has(patch.segment.id)) continue;
    existingIds.add(patch.segment.id);
    added += 1;
  }
  const segments = trimRecordSegments(
    sortRecordSegments(applySubtitlePatches(snapshot.segments, batch.patches)),
    snapshot.historyLimit,
  );
  return {
    ...snapshot,
    subtitleRevision: batch.revision,
    segments,
    totalSubtitleCount: snapshot.totalSubtitleCount + added,
  };
}

export function mergeRecordSnapshot(
  current: RecordSnapshot | undefined,
  incoming: RecordSnapshot,
): RecordSnapshot {
  if (!current) return incoming;
  let summary = incoming.summary;
  if (current.summaryRevision > incoming.summaryRevision) summary = current.summary;
  let segments = incoming.segments;
  if (current.subtitleRevision > incoming.subtitleRevision) segments = current.segments;
  let stats = incoming.stats;
  if (current.statsRevision > incoming.statsRevision) stats = current.stats;
  let notices = incoming.notices;
  if (current.summary.updatedAtMs > incoming.summary.updatedAtMs) notices = current.notices;
  return {
    ...incoming,
    summary,
    segments,
    stats,
    notices,
    summaryRevision: Math.max(incoming.summaryRevision, current.summaryRevision),
    subtitleRevision: Math.max(incoming.subtitleRevision, current.subtitleRevision),
    statsRevision: Math.max(incoming.statsRevision, current.statsRevision),
    totalSubtitleCount: Math.max(incoming.totalSubtitleCount, current.totalSubtitleCount),
    historyLimit: incoming.historyLimit,
  };
}

export function sortRecordSegments(segments: SubtitleSegment[]): SubtitleSegment[] {
  return segments.toSorted((left, right) => {
    if (left.occurrenceMs !== right.occurrenceMs) return left.occurrenceMs - right.occurrenceMs;
    return left.id.localeCompare(right.id);
  });
}

export function isRecordActiveState(state: RecordState): boolean {
  return state === 'starting' || state === 'running' || state === 'waitingForDevice';
}

function trimRecordSegments(segments: SubtitleSegment[], historyLimit: number): SubtitleSegment[] {
  if (historyLimit <= 0 || segments.length <= historyLimit) return segments;
  return segments.slice(segments.length - historyLimit);
}

function isRecordSnapshot(value: unknown): value is RecordSnapshot {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Partial<RecordSnapshot>;
  return (
    typeof candidate.summaryRevision === 'number' &&
    typeof candidate.subtitleRevision === 'number' &&
    typeof candidate.statsRevision === 'number' &&
    typeof candidate.totalSubtitleCount === 'number' &&
    typeof candidate.historyLimit === 'number' &&
    Array.isArray(candidate.segments) &&
    Array.isArray(candidate.notices) &&
    typeof candidate.summary === 'object' &&
    candidate.summary !== null
  );
}
