import { useEffect, useRef } from 'react';
import { Icon } from '@iconify/react';
import captionsIcon from '@iconify-icons/lucide/captions';
import loaderCircleIcon from '@iconify-icons/lucide/loader-circle';
import radioIcon from '@iconify-icons/lucide/radio';
import userRoundIcon from '@iconify-icons/lucide/user-round';
import { useTranslation } from '@sonasub/i18n';
import { Badge } from '@sonasub/ui/components/badge';
import { Card, CardContent, CardHeader, CardTitle } from '@sonasub/ui/components/card';
import { useQuery } from '@tanstack/react-query';

import { formatAudioTime } from '@/lib/format';
import { sessionSnapshotOptions } from '@/lib/session';
import type { SubtitleSegment } from '@/lib/session.type';

const EMPTY_SEGMENTS: SubtitleSegment[] = [];

function TranslationBody({ segment }: { segment: SubtitleSegment }) {
  const { t } = useTranslation();
  const translation = segment.translation;
  if (!translation) return null;

  if (translation.status === 'ready' && translation.text) {
    return <p className="text-[15px] leading-7 text-foreground">{translation.text}</p>;
  }
  if (translation.status === 'pending') {
    return (
      <p className="flex items-center gap-2 text-sm text-muted-foreground">
        <Icon icon={loaderCircleIcon} className="size-3.5 animate-spin" />
        {t(($) => $.captions.translation.pending)}
      </p>
    );
  }
  if (translation.status === 'stale') {
    return (
      <p className="flex items-center gap-2 text-sm text-muted-foreground">
        <Icon icon={loaderCircleIcon} className="size-3.5 animate-spin" />
        {t(($) => $.captions.translation.stale)}
      </p>
    );
  }
  if (translation.status === 'notNeeded') {
    return (
      <p className="text-sm text-muted-foreground">{t(($) => $.captions.translation.notNeeded)}</p>
    );
  }
  if (translation.status === 'unavailable') {
    return (
      <p className="text-sm text-warning">
        {t(($) => $.captions.translation.unavailable)}
        {translation.error && <span className="ml-1">{translation.error}</span>}
      </p>
    );
  }
  if (translation.status === 'error') {
    return (
      <p className="text-sm text-destructive">
        {t(($) => $.captions.translation.error)}
        {translation.error && <span className="ml-1">{translation.error}</span>}
      </p>
    );
  }
  return null;
}

function SubtitleRow({ segment }: { segment: SubtitleSegment }) {
  const { t } = useTranslation();
  const speaker = segment.speaker;
  let speakerLabel: string = t(($) => $.captions.speakerPending);
  if (speaker) {
    speakerLabel = t(($) => $.captions.speaker, { index: speaker.index });
  }

  let rowTone = 'border-border bg-card';
  if (!segment.isFinal) rowTone = 'border-warning/40 bg-warning/5';

  return (
    <li className={`rounded-xl border p-4 transition-colors ${rowTone}`}>
      <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <time dateTime={`PT${segment.startSecs.toFixed(1)}S`} className="font-mono tabular-nums">
          {formatAudioTime(segment.startSecs)}
        </time>
        <Badge variant="secondary" className="gap-1">
          <Icon icon={userRoundIcon} />
          {speakerLabel}
        </Badge>
        {speaker?.uncertain && <Badge variant="outline">{t(($) => $.captions.uncertain)}</Badge>}
        {speaker?.overlap && <Badge variant="outline">{t(($) => $.captions.overlap)}</Badge>}
        {!segment.isFinal && (
          <Badge variant="outline" className="border-warning/50 text-warning">
            <span className="size-1.5 rounded-full bg-warning" />
            {t(($) => $.captions.partial)}
          </Badge>
        )}
      </div>
      <p className="mt-3 text-lg leading-8 font-medium text-foreground">{segment.text}</p>
      <div className="mt-2 min-h-6 border-l-2 border-border pl-3">
        <TranslationBody segment={segment} />
      </div>
    </li>
  );
}

export function LiveSubtitles() {
  const { t } = useTranslation();
  const { data: snapshot, isPending } = useQuery(sessionSnapshotOptions);
  const endRef = useRef<HTMLDivElement>(null);
  const segments = snapshot?.segments ?? EMPTY_SEGMENTS;
  const isRunning = snapshot?.status.state === 'running';

  useEffect(() => {
    if (segments.length === 0) return;
    endRef.current?.scrollIntoView?.({ block: 'end' });
  }, [segments.length]);

  let emptyTitle: string = t(($) => $.captions.emptyTitle);
  let emptyDescription: string = t(($) => $.captions.emptyDescription);
  if (isRunning) {
    emptyTitle = t(($) => $.captions.listeningTitle);
    emptyDescription = t(($) => $.captions.listeningDescription);
  }

  return (
    <Card className="flex min-h-[580px] flex-col overflow-hidden">
      <CardHeader className="border-b">
        <div className="flex items-center justify-between gap-3">
          <CardTitle className="flex items-center gap-2 text-base">
            <Icon icon={captionsIcon} className="size-4 text-muted-foreground" />
            {t(($) => $.captions.title)}
          </CardTitle>
          {isRunning && (
            <Badge variant="outline" className="gap-1 border-success/40 text-success">
              <Icon icon={radioIcon} className="animate-pulse" />
              {t(($) => $.workbench.status.running)}
            </Badge>
          )}
        </div>
        <p className="text-sm text-muted-foreground">
          {t(($) => $.captions.count, { count: segments.length })}
        </p>
      </CardHeader>
      <CardContent className="min-h-0 flex-1 p-0">
        <div className="h-[500px] overflow-y-auto px-4 py-4 sm:px-5">
          {isPending && (
            <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
              {t(($) => $.workbench.status.loading)}
            </div>
          )}
          {!isPending && segments.length === 0 && (
            <div className="flex h-full flex-col items-center justify-center px-6 text-center">
              <div className="mb-4 grid size-12 place-items-center rounded-full border bg-muted/40">
                <Icon icon={captionsIcon} className="size-5 text-muted-foreground" />
              </div>
              <p className="text-sm font-medium">{emptyTitle}</p>
              <p className="mt-2 max-w-md text-sm leading-6 text-muted-foreground">
                {emptyDescription}
              </p>
            </div>
          )}
          <ol aria-live="polite" className="space-y-3">
            {segments.map((segment) => (
              <SubtitleRow key={segment.id} segment={segment} />
            ))}
          </ol>
          <div ref={endRef} />
        </div>
      </CardContent>
    </Card>
  );
}
