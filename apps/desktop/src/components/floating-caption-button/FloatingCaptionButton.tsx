import { useEffect, useState } from 'react';
import { Icon } from '@iconify/react';
import lockOpenIcon from '@iconify-icons/lucide/lock-open';
import monitorUpIcon from '@iconify-icons/lucide/monitor-up';
import xIcon from '@iconify-icons/lucide/x';
import { useTranslation } from '@sonasub/i18n';
import { Button } from '@sonasub/ui/components/button';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';

import {
  CAPTION_OVERLAY_CLOSED_EVENT,
  CAPTION_OVERLAY_OPEN_EVENT,
  LOCK_EVENT,
} from '@/lib/caption-overlay';
import type { CaptionOverlayLockState } from '@/lib/caption-overlay';

import type { FloatingCaptionButtonProps } from './FloatingCaptionButton.type';

export function FloatingCaptionButton({ recordId }: FloatingCaptionButtonProps) {
  const { t } = useTranslation();
  const [isLocked, setIsLocked] = useState(false);
  const [isOpen, setIsOpen] = useState(true);

  useEffect(() => {
    let isDisposed = false;
    const unlisteners: UnlistenFn[] = [];
    function track(unlisten: UnlistenFn) {
      if (isDisposed) unlisten();
      else unlisteners.push(unlisten);
    }
    void listen<CaptionOverlayLockState>(LOCK_EVENT, ({ payload }) => {
      setIsLocked(payload.locked);
      setIsOpen(!payload.locked);
    }).then(track);
    void listen(CAPTION_OVERLAY_OPEN_EVENT, () => setIsOpen(true)).then(track);
    void listen(CAPTION_OVERLAY_CLOSED_EVENT, () => setIsOpen(false)).then(track);
    void invoke<boolean>('caption_overlay_is_locked')
      .then((locked) => {
        setIsLocked(locked);
        setIsOpen(!locked);
      })
      .catch(() => undefined);
    return () => {
      isDisposed = true;
      for (const unlisten of unlisteners) unlisten();
    };
  }, []);

  if (isLocked) {
    return (
      <div className="flex shrink-0 items-center gap-1">
        <Button
          type="button"
          variant="outline"
          size="sm"
          title={t(($) => $.captions.floating.unlock)}
          onClick={() => void invoke('set_caption_overlay_locked', { locked: false })}
        >
          <Icon icon={lockOpenIcon} />
          {t(($) => $.captions.floating.unlock)}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          title={t(($) => $.captions.floating.close)}
          aria-label={t(($) => $.captions.floating.close)}
          onClick={() => void invoke('close_caption_overlay')}
        >
          <Icon icon={xIcon} />
        </Button>
      </div>
    );
  }

  return (
    <div className="flex shrink-0 items-center gap-1">
      <Button
        type="button"
        variant="outline"
        size="sm"
        title={t(($) => $.captions.floating.open)}
        onClick={() => void invoke('open_caption_overlay', { recordId })}
      >
        <Icon icon={monitorUpIcon} />
        {t(($) => $.captions.floating.open)}
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        title={t(($) => $.captions.floating.close)}
        aria-label={t(($) => $.captions.floating.close)}
        disabled={!isOpen}
        onClick={() => void invoke('close_caption_overlay')}
      >
        <Icon icon={xIcon} />
      </Button>
    </div>
  );
}
