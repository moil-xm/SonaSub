import { Icon } from '@iconify/react';
import captionsIcon from '@iconify-icons/lucide/captions';
import lockIcon from '@iconify-icons/lucide/lock';
import xIcon from '@iconify-icons/lucide/x';
import { useTranslation } from '@sonasub/i18n';
import { Button } from '@sonasub/ui/components/button';
import { useTheme } from '@sonasub/ui/theme';
import { invoke } from '@tauri-apps/api/core';
import { useQuery } from '@tanstack/react-query';

import { TranslationBody } from '@/components/live-subtitles';
import {
  selectOverlaySegment,
  useCaptionOverlayState,
  useOverlayRecordEvents,
} from '@/lib/caption-overlay';
import { recordSnapshotOptions } from '@/lib/records';
import { useCaptionTextSize } from '@/stores/app-preferences';

export function FloatingCaptionCard() {
  const { t } = useTranslation();
  useTheme();
  useCaptionTextSize();
  const { boundRecordId, isLocked } = useCaptionOverlayState();
  useOverlayRecordEvents(boundRecordId);

  const snapshotQuery = useQuery({
    ...recordSnapshotOptions(boundRecordId ?? '__none__'),
    enabled: boundRecordId !== null,
    refetchOnWindowFocus: false,
  });
  const snapshot = snapshotQuery.data;
  const segment = snapshot ? selectOverlaySegment(snapshot) : null;
  const isUnavailable =
    snapshotQuery.error !== null || (snapshotQuery.data === undefined && !snapshotQuery.isPending);
  const status = snapshot?.summary.state;
  let statusLabel: string | null = null;
  if (status === 'paused') statusLabel = t(($) => $.captions.floating.paused);
  if (status === 'ended') statusLabel = t(($) => $.captions.floating.ended);
  if (status === 'failed') statusLabel = t(($) => $.captions.floating.failed);
  if (status === 'waitingForDevice') statusLabel = t(($) => $.captions.floating.waitingForDevice);
  if (status === 'notStarted') statusLabel = t(($) => $.captions.floating.notStarted);
  if (status === 'starting') statusLabel = t(($) => $.captions.floating.starting);

  return (
    <main
      className="flex h-screen w-screen select-none flex-col overflow-hidden bg-background/92 text-foreground backdrop-blur"
      data-tauri-drag-region={!isLocked}
    >
      <header
        data-tauri-drag-region={!isLocked}
        className="flex h-8 shrink-0 items-center justify-between gap-2 border-b border-border px-3"
      >
        <div className="flex min-w-0 items-center gap-2">
          <Icon icon={captionsIcon} className="size-3.5 shrink-0 text-muted-foreground" />
          <span className="truncate text-[11px] font-medium">
            {t(($) => $.captions.floating.title)}
          </span>
        </div>
        {!isLocked && (
          <div className="flex shrink-0 items-center gap-1">
            <Button
              type="button"
              variant="ghost"
              size="icon"
              title={t(($) => $.captions.floating.lock)}
              aria-label={t(($) => $.captions.floating.lock)}
              onClick={() => void invoke('set_caption_overlay_locked', { locked: true })}
            >
              <Icon icon={lockIcon} className="size-3.5" />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              title={t(($) => $.captions.floating.close)}
              aria-label={t(($) => $.captions.floating.close)}
              onClick={() => void invoke('close_caption_overlay')}
            >
              <Icon icon={xIcon} className="size-3.5" />
            </Button>
          </div>
        )}
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        {statusLabel && (
          <p className="mb-2 text-[10px] font-medium text-muted-foreground">{statusLabel}</p>
        )}
        {isUnavailable && (
          <p className="text-sm text-muted-foreground">
            {t(($) => $.captions.floating.recordUnavailable)}
          </p>
        )}
        {!isUnavailable && !segment && (
          <p className="text-sm text-muted-foreground">{t(($) => $.captions.floating.waiting)}</p>
        )}
        {!isUnavailable && segment && (
          <div className="space-y-2">
            <p className="caption-source break-words">{segment.text}</p>
            <TranslationBody segment={segment} />
          </div>
        )}
      </div>
    </main>
  );
}
