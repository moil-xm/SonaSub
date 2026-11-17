import { useEffect, useRef, useState } from 'react';
import { Icon } from '@iconify/react';
import arrowUpIcon from '@iconify-icons/lucide/arrow-up';
import captionsIcon from '@iconify-icons/lucide/captions';
import radioIcon from '@iconify-icons/lucide/radio';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import { useTranslation } from '@sonasub/i18n';
import { Badge } from '@sonasub/ui/components/badge';
import { useQuery } from '@tanstack/react-query';

import { sessionSnapshotOptions } from '@/lib/session';
import type { SubtitleSegment } from '@/lib/session';

import { SubtitleRow } from './SubtitleRow';

const EMPTY_SEGMENTS: SubtitleSegment[] = [];
const FOLLOW_THRESHOLD_PX = 48;

export function LiveSubtitles() {
  const { t } = useTranslation();
  const { data: snapshot, isPending, error } = useQuery(sessionSnapshotOptions);
  const viewportRef = useRef<HTMLDivElement>(null);
  const segments = snapshot?.segments ?? EMPTY_SEGMENTS;
  const isRunning = snapshot?.status.state === 'running';
  const [followState, setFollowState] = useState({
    sessionId: '',
    isFollowing: true,
  });
  let sessionId = '';
  if (snapshot?.status.state === 'running') sessionId = snapshot.status.info.sessionId;
  const isFollowing = sessionId === followState.sessionId ? followState.isFollowing : true;

  useEffect(() => {
    if (!isFollowing || segments.length === 0) return;
    const viewport = viewportRef.current;
    if (!viewport) return;
    viewport.scrollTop = viewport.scrollHeight;
  }, [isFollowing, segments]);

  function handleScroll() {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const distanceToEnd = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
    setFollowState({
      sessionId,
      isFollowing: distanceToEnd < FOLLOW_THRESHOLD_PX,
    });
  }

  function jumpToLatest() {
    const viewport = viewportRef.current;
    if (!viewport) return;
    setFollowState({ sessionId, isFollowing: true });
    viewport.scrollTop = viewport.scrollHeight;
  }

  let emptyTitle: string = t(($) => $.captions.emptyTitle);
  let emptyDescription: string = t(($) => $.captions.emptyDescription);
  let emptyIcon = captionsIcon;
  if (isRunning) {
    emptyTitle = t(($) => $.captions.listeningTitle);
    emptyDescription = t(($) => $.captions.listeningDescription);
    emptyIcon = radioIcon;
  }

  return (
    <section className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border bg-card">
      <header className="flex h-[46px] shrink-0 items-center justify-between gap-3 border-b px-4">
        <div className="flex min-w-0 items-center gap-2">
          <Icon icon={captionsIcon} className="size-4 shrink-0 text-muted-foreground" />
          <h2 className="truncate text-[13px] font-medium">{t(($) => $.captions.title)}</h2>
          <span className="text-[11.5px] text-muted-foreground">
            {t(($) => $.captions.count, { count: segments.length })}
          </span>
        </div>
        {isRunning && (
          <Badge variant="outline" className="gap-1 border-success/40 text-success">
            <Icon icon={radioIcon} className="animate-pulse" />
            {t(($) => $.workbench.status.running)}
          </Badge>
        )}
      </header>

      <div className="relative min-h-0 flex-1">
        <div ref={viewportRef} className="h-full overflow-y-auto" onScroll={handleScroll}>
          {isPending && (
            <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
              {t(($) => $.workbench.status.loading)}
            </div>
          )}
          {!isPending && error && (
            <div className="flex h-full flex-col items-center justify-center px-6 text-center">
              <Icon icon={triangleAlertIcon} className="size-5 text-destructive" />
              <p className="mt-3 text-sm font-medium">{t(($) => $.workbench.errors.snapshot)}</p>
              <p className="mt-1 max-w-md text-xs leading-5 text-muted-foreground">
                {error.message}
              </p>
            </div>
          )}
          {!isPending && !error && segments.length === 0 && (
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
                <SubtitleRow key={segment.id} segment={segment} />
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
