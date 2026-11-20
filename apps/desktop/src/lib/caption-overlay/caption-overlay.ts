import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useQueryClient } from '@tanstack/react-query';

import { applyRecordSubtitleBatch, recordSnapshotOptions } from '@/lib/records';
import type {
  RecordNoticeEvent,
  RecordSnapshot,
  RecordStatsEvent,
  RecordSubtitleBatch,
  RecordSummaryEvent,
} from '@/lib/records';
import type { SubtitleSegment } from '@/lib/session';

import type { CaptionOverlayBinding, CaptionOverlayLockState } from './caption-overlay.type';

export const BOUND_RECORD_EVENT = 'overlay://bound-record' as const;
export const LOCK_EVENT = 'overlay://lock' as const;
export const CAPTION_OVERLAY_OPEN_EVENT = 'overlay://open' as const;
export const CAPTION_OVERLAY_CLOSED_EVENT = 'overlay://closed' as const;

const RECORD_SUMMARY_EVENT = 'record://summary' as const;
const RECORD_SUBTITLES_EVENT = 'record://subtitles' as const;
const RECORD_STATS_EVENT = 'record://stats' as const;
const RECORD_NOTICE_EVENT = 'record://notice' as const;

type CaptionOverlayState = {
  boundRecordId: string | null;
  isLocked: boolean;
};

export function useCaptionOverlayState(): CaptionOverlayState {
  const [state, setState] = useState<CaptionOverlayState>({
    boundRecordId: null,
    isLocked: false,
  });

  useEffect(() => {
    let isDisposed = false;
    const unlisteners: UnlistenFn[] = [];
    function track(unlisten: UnlistenFn) {
      if (isDisposed) unlisten();
      else unlisteners.push(unlisten);
    }
    void listen<CaptionOverlayBinding>(BOUND_RECORD_EVENT, ({ payload }) => {
      setState((current) => ({ ...current, boundRecordId: payload.recordId }));
    }).then(track);
    void listen<CaptionOverlayLockState>(LOCK_EVENT, ({ payload }) => {
      setState((current) => ({ ...current, isLocked: payload.locked }));
    }).then(track);
    void Promise.all([
      invoke<string | null>('caption_overlay_binding'),
      invoke<CaptionOverlayLockState>('caption_overlay_lock_state'),
    ])
      .then(([recordId, lockState]) => {
        if (isDisposed) return;
        setState({ boundRecordId: recordId, isLocked: lockState.locked });
      })
      .catch(() => undefined);
    return () => {
      isDisposed = true;
      for (const unlisten of unlisteners) unlisten();
    };
  }, []);

  return state;
}

export function useOverlayRecordEvents(recordId: string | null) {
  const queryClient = useQueryClient();

  useEffect(() => {
    if (!recordId) return undefined;
    const snapshotKey = recordId;
    let isDisposed = false;
    const unlisteners: UnlistenFn[] = [];
    function track(unlisten: UnlistenFn) {
      if (isDisposed) unlisten();
      else unlisteners.push(unlisten);
    }

    function updateSnapshot(updater: (current: RecordSnapshot) => RecordSnapshot) {
      queryClient.setQueryData<RecordSnapshot>(
        recordSnapshotOptions(snapshotKey).queryKey,
        (current) => {
          if (!current) return current;
          return updater(current);
        },
      );
    }

    const listeners = [
      listen<RecordSummaryEvent>(RECORD_SUMMARY_EVENT, ({ payload }) => {
        if (payload.recordId !== snapshotKey) return;
        updateSnapshot((current) => {
          if (payload.revision < current.summaryRevision) return current;
          return {
            ...current,
            summary: payload.summary,
            summaryRevision: payload.revision,
          };
        });
      }).then(track),
      listen<RecordSubtitleBatch>(RECORD_SUBTITLES_EVENT, ({ payload }) => {
        if (payload.recordId !== snapshotKey) return;
        updateSnapshot((current) => applyRecordSubtitleBatch(current, payload));
      }).then(track),
      listen<RecordStatsEvent>(RECORD_STATS_EVENT, ({ payload }) => {
        if (payload.recordId !== snapshotKey) return;
        updateSnapshot((current) => {
          if (payload.revision <= current.statsRevision) return current;
          return { ...current, stats: payload.stats, statsRevision: payload.revision };
        });
      }).then(track),
      listen<RecordNoticeEvent>(RECORD_NOTICE_EVENT, ({ payload }) => {
        if (payload.recordId !== snapshotKey) return;
        updateSnapshot((current) => ({
          ...current,
          notices: [...current.notices, payload.notice].slice(-100),
        }));
      }).then(track),
    ];

    void Promise.allSettled(listeners).then(() => {
      if (isDisposed) return;
      void queryClient.invalidateQueries({ queryKey: recordSnapshotOptions(snapshotKey).queryKey });
    });

    return () => {
      isDisposed = true;
      for (const unlisten of unlisteners) unlisten();
    };
  }, [queryClient, recordId]);
}

export function selectOverlaySegment(snapshot: RecordSnapshot): SubtitleSegment | null {
  let latest: SubtitleSegment | null = null;
  for (const segment of snapshot.segments) {
    if (!latest || segment.occurrenceMs > latest.occurrenceMs) latest = segment;
  }
  return latest;
}
