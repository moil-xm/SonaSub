import { useEffect, useState } from 'react';
import { Icon } from '@iconify/react';
import captionsIcon from '@iconify-icons/lucide/captions';
import lockIcon from '@iconify-icons/lucide/lock';
import xIcon from '@iconify-icons/lucide/x';
import { useTranslation } from '@sonasub/i18n';
import { Button } from '@sonasub/ui/components/button';
import { useTheme } from '@sonasub/ui/theme';
import { useQuery } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';

import { BOUND_RECORD_EVENT, LOCK_EVENT, selectOverlaySegment } from '@/lib/caption-overlay';
import type { CaptionOverlayBinding, CaptionOverlayLockState } from '@/lib/caption-overlay';
import { recordSnapshotOptions, useRecordEvents } from '@/lib/records';
import { useCaptionTextSize } from '@/stores/app-preferences';

export function FloatingCaptionCard() {
  const { t } = useTranslation();
  useTheme();
  useCaptionTextSize();
  const [boundRecordId, setBoundRecordId] = useState<string | null>(null);
  const [isLocked, setIsLocked] = useState(false);
  useRecordEvents(boundRecordId ?? undefined);

  useEffect(() => {
    let isDisposed = false;
    const unlisteners: UnlistenFn[] = [];
    function track(unlisten: UnlistenFn) {
      if (isDisposed) unlisten();
      else unlisteners.push(unlisten);
    }
    void listen<CaptionOverlayBinding>(BOUND_RECORD_EVENT, ({ payload }) => {
      setBoundRecordId(payload.recordId);
    }).then(track);
    void listen<CaptionOverlayLockState>(LOCK_EVENT, ({ payload }) => {
      setIsLocked(payload.locked);
    }).then(track);
    void invoke<string | null>('caption_overlay_binding')
      .then((recordId) => {
        setBoundRecordId(recordId);
      })
      .catch(() => undefined);
    void invoke<boolean>('caption_overlay_is_locked')
      .then(setIsLocked)
      .catch(() => undefined);
    return () => {
      isDisposed = true;
      for (const unlisten of unlisteners) unlisten();
    };
  }, []);

  const snapshotQuery = useQuery({
    ...recordSnapshotOptions(boundRecordId ?? '__none__'),
    enabled: boundRecordId !== null,
  });
  const snapshot = snapshotQuery.data;
  const segment = snapshot ? selectOverlaySegment(snapshot) : null;
  const translation = segment?.translation;
  const isUnavailable =
    snapshotQuery.error !== null || (snapshotQuery.data === undefined && !snapshotQuery.isPending);

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
      <div className="min-h-0 flex-1 overflow-y-auto p-4">
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
            {translation?.status === 'ready' && translation.text && (
              <p className="caption-translation break-words text-foreground/80">
                {translation.text}
              </p>
            )}
            {translation?.status === 'pending' && (
              <p className="text-xs text-muted-foreground">
                {t(($) => $.captions.translation.pending)}
              </p>
            )}
            {translation?.status === 'stale' && (
              <p className="text-xs text-muted-foreground">
                {t(($) => $.captions.translation.stale)}
              </p>
            )}
          </div>
        )}
      </div>
    </main>
  );
}
