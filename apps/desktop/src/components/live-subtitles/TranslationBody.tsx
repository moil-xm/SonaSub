import { Icon } from '@iconify/react';
import loaderCircleIcon from '@iconify-icons/lucide/loader-circle';
import { useTranslation } from '@sonasub/i18n';

import type { SubtitleSegment } from '@/lib/session.type';

type TranslationBodyProps = {
  segment: SubtitleSegment;
};

export function TranslationBody({ segment }: TranslationBodyProps) {
  const { t } = useTranslation();
  const translation = segment.translation;
  if (!translation) return null;

  if (translation.status === 'ready' && translation.text) {
    return <p className="caption-translation text-foreground/80">{translation.text}</p>;
  }
  if (translation.status === 'pending') {
    return (
      <p className="flex items-center gap-2 text-xs text-muted-foreground">
        <Icon icon={loaderCircleIcon} className="size-3.5 animate-spin" />
        {t(($) => $.captions.translation.pending)}
      </p>
    );
  }
  if (translation.status === 'stale') {
    return (
      <p className="flex items-center gap-2 text-xs text-muted-foreground">
        <Icon icon={loaderCircleIcon} className="size-3.5 animate-spin" />
        {t(($) => $.captions.translation.stale)}
      </p>
    );
  }
  if (translation.status === 'notNeeded') {
    return (
      <p className="text-xs text-muted-foreground">{t(($) => $.captions.translation.notNeeded)}</p>
    );
  }
  if (translation.status === 'unavailable') {
    return (
      <p className="text-xs text-warning">
        {t(($) => $.captions.translation.unavailable)}
        {translation.error && <span className="ml-1">{translation.error}</span>}
      </p>
    );
  }
  if (translation.status === 'error') {
    return (
      <p className="text-xs text-destructive">
        {t(($) => $.captions.translation.error)}
        {translation.error && <span className="ml-1">{translation.error}</span>}
      </p>
    );
  }
  return null;
}
