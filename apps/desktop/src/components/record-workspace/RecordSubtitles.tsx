import { useEffect, useRef, useState } from 'react';
import { Icon } from '@iconify/react';
import arrowUpIcon from '@iconify-icons/lucide/arrow-up';
import captionsIcon from '@iconify-icons/lucide/captions';
import loaderCircleIcon from '@iconify-icons/lucide/loader-circle';
import radioIcon from '@iconify-icons/lucide/radio';
import { useTranslation } from '@sonasub/i18n';
import { Button } from '@sonasub/ui/components/button';
import { useInfiniteQuery } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';

import { SubtitleRow } from '@/components/live-subtitles';
import type { RecordState, RecordSubtitlePage, SubtitleCursor } from '@/lib/records';
import { sortRecordSegments } from '@/lib/records';
import type { SubtitleSegment } from '@/lib/session';

const FOLLOW_THRESHOLD_PX = 48;

type RecordSubtitlesProps = {
  initialSegments: SubtitleSegment[];
  recordId: string;
  showSpeaker: boolean;
  state: RecordState;
  totalCount: number;
};

export function RecordSubtitles({
  initialSegments,
  recordId,
  showSpeaker,
  state,
  totalCount,
}: RecordSubtitlesProps) {
  const { t } = useTranslation();
  const viewportRef = useRef<HTMLDivElement>(null);
  const [isFollowing, setIsFollowing] = useState(true);
  const [isHistoryOpen, setIsHistoryOpen] = useState(false);
  const oldest = initialSegments[0];
  let initialCursor: SubtitleCursor | null = null;
  if (oldest) {
    initialCursor = { occurrenceMs: oldest.occurrenceMs, id: oldest.id };
  }
  const history = useInfiniteQuery({
    queryKey: [
      'records',
      recordId,
      'history',
      initialCursor?.occurrenceMs ?? 0,
      initialCursor?.id ?? '',
    ],
    queryFn: ({ pageParam }) =>
      invoke<RecordSubtitlePage>('list_record_subtitles', {
        recordId,
        before: pageParam,
        limit: 100,
      }),
    initialPageParam: initialCursor,
    getNextPageParam: (lastPage) => {
      if (!lastPage.hasMore) return undefined;
      return lastPage.nextBefore ?? undefined;
    },
    enabled: isHistoryOpen,
    networkMode: 'always',
    retry: false,
  });

  useEffect(() => {
    if (!isFollowing || initialSegments.length === 0) return;
    const viewport = viewportRef.current;
    if (!viewport) return;
    viewport.scrollTop = viewport.scrollHeight;
  }, [initialSegments, isFollowing]);

  const olderPages = history.data?.pages.toReversed() ?? [];
  const olderItems = olderPages.flatMap((page) => page.items);
  const byId = new Map<string, SubtitleSegment>();
  for (const segment of olderItems) byId.set(segment.id, segment);
  for (const segment of initialSegments) byId.set(segment.id, segment);
  const segments = sortRecordSegments([...byId.values()]);
  const hasStoredHistory = totalCount > initialSegments.length;
  const hasMore = history.hasNextPage || !isHistoryOpen;
  const isLoadingHistory = history.isFetching;
  let loadOlderLabel: string = t(($) => $.records.workspace.loadOlder);
  if (isLoadingHistory) loadOlderLabel = t(($) => $.records.workspace.loadingOlder);

  function handleScroll() {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const distanceToEnd = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
    setIsFollowing(distanceToEnd < FOLLOW_THRESHOLD_PX);
  }

  function jumpToLatest() {
    const viewport = viewportRef.current;
    if (!viewport) return;
    setIsFollowing(true);
    viewport.scrollTop = viewport.scrollHeight;
  }

  function loadOlder() {
    if (!isHistoryOpen) {
      setIsHistoryOpen(true);
      return;
    }
    void history.fetchNextPage();
  }

  let emptyTitle: string = t(($) => $.records.workspace.emptyTitle);
  let emptyDescription: string = t(($) => $.records.workspace.emptyDescription);
  let emptyIcon = captionsIcon;
  if (state === 'running') {
    emptyTitle = t(($) => $.records.workspace.listeningTitle);
    emptyDescription = t(($) => $.records.workspace.listeningDescription);
    emptyIcon = radioIcon;
  }
  if (state === 'waitingForDevice') {
    emptyTitle = t(($) => $.records.status.waitingForDevice);
    emptyDescription = t(($) => $.records.workspace.waitingForDevice);
    emptyIcon = radioIcon;
  }
  if (state === 'failed') {
    emptyTitle = t(($) => $.records.status.failed);
    emptyDescription = t(($) => $.records.workspace.failed);
  }
  if (state === 'ended') {
    emptyTitle = t(($) => $.records.status.ended);
    emptyDescription = t(($) => $.records.workspace.ended);
  }

  return (
    <section className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border bg-card">
      <header className="flex h-[46px] shrink-0 items-center justify-between gap-3 border-b px-4">
        <div className="flex min-w-0 items-center gap-2">
          <Icon icon={captionsIcon} className="size-4 shrink-0 text-muted-foreground" />
          <h2 className="truncate text-[13px] font-medium">{t(($) => $.captions.title)}</h2>
          <span className="text-[11.5px] text-muted-foreground">
            {t(($) => $.records.workspace.count, { count: totalCount })}
          </span>
        </div>
        {state === 'running' && (
          <span className="inline-flex items-center gap-1.5 rounded-full border border-success/40 px-2 py-0.5 text-[10.5px] font-medium text-success">
            <Icon icon={radioIcon} className="size-3 animate-pulse" />
            {t(($) => $.workbench.status.running)}
          </span>
        )}
      </header>

      <div className="relative min-h-0 flex-1">
        <div ref={viewportRef} className="h-full overflow-y-auto" onScroll={handleScroll}>
          {(hasStoredHistory || isHistoryOpen) && (
            <div className="flex justify-center border-b border-border/70 px-4 py-2">
              {hasMore && (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  disabled={isLoadingHistory}
                  onClick={loadOlder}
                >
                  {isLoadingHistory && <Icon icon={loaderCircleIcon} className="animate-spin" />}
                  {loadOlderLabel}
                </Button>
              )}
              {!hasMore && history.data && (
                <span className="py-1 text-[11px] text-muted-foreground">
                  {t(($) => $.records.workspace.allLoaded)}
                </span>
              )}
            </div>
          )}

          {segments.length === 0 && (
            <div className="flex h-full min-h-[300px] flex-col items-center justify-center px-6 text-center">
              <div className="mb-3 grid size-10 place-items-center rounded-full bg-muted text-muted-foreground">
                <Icon icon={emptyIcon} className="size-4" />
              </div>
              <p className="text-[13px] font-medium">{emptyTitle}</p>
              <p className="mt-1.5 max-w-md text-xs leading-5 text-muted-foreground">
                {emptyDescription}
              </p>
            </div>
          )}

          {segments.length > 0 && (
            <ol aria-live="polite" className="divide-y divide-border/70">
              {segments.map((segment) => (
                <SubtitleRow key={segment.id} segment={segment} showSpeaker={showSpeaker} />
              ))}
            </ol>
          )}
        </div>
        {!isFollowing && segments.length > 0 && (
          <button
            type="button"
            className="absolute right-4 bottom-4 inline-flex items-center gap-1.5 rounded-full border bg-popover px-3 py-1.5 text-xs font-medium text-foreground shadow-md transition-colors hover:bg-muted"
            onClick={jumpToLatest}
          >
            <Icon icon={arrowUpIcon} className="size-3.5" />
            {t(($) => $.captions.jumpToLatest)}
          </button>
        )}
      </div>
    </section>
  );
}
