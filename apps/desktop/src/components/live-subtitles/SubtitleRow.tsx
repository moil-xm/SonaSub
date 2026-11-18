import { Icon } from '@iconify/react';
import speakerIcon from '@iconify-icons/lucide/speaker';
import userRoundIcon from '@iconify-icons/lucide/user-round';
import { useTranslation } from '@sonasub/i18n';
import { Badge } from '@sonasub/ui/components/badge';

import { formatAudioTime } from '@/lib/format';
import type { SubtitleSegment } from '@/lib/session';

import { TranslationBody } from './TranslationBody';

type SubtitleRowProps = {
  segment: SubtitleSegment;
  showSpeaker?: boolean;
};

export function SubtitleRow({ segment, showSpeaker = true }: SubtitleRowProps) {
  const { t } = useTranslation();
  const speaker = segment.speaker;
  let speakerLabel: string = t(($) => $.captions.speakerPending);
  if (speaker) {
    speakerLabel = t(($) => $.captions.speaker, { index: speaker.index });
  }

  let rowClass = 'px-4 py-4 transition-colors sm:px-5';
  if (!segment.isFinal) rowClass = 'bg-warning/[0.035] px-4 py-4 transition-colors sm:px-5';

  return (
    <li className={rowClass}>
      <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-muted-foreground">
        <time
          dateTime={`PT${segment.deviceTimeSecs.toFixed(1)}S`}
          className="font-mono tabular-nums"
        >
          {formatAudioTime(segment.deviceTimeSecs)}
        </time>
        {segment.deviceName && (
          <span className="inline-flex min-w-0 items-center gap-1 rounded bg-muted px-1.5 py-0.5">
            <Icon icon={speakerIcon} className="size-3 shrink-0" />
            <span className="max-w-[220px] truncate">{segment.deviceName}</span>
          </span>
        )}
        {showSpeaker && (
          <span className="inline-flex items-center gap-1 rounded bg-muted px-1.5 py-0.5">
            <Icon icon={userRoundIcon} className="size-3" />
            {speakerLabel}
          </span>
        )}
        {speaker?.uncertain && <Badge variant="outline">{t(($) => $.captions.uncertain)}</Badge>}
        {speaker?.overlap && <Badge variant="outline">{t(($) => $.captions.overlap)}</Badge>}
        {!segment.isFinal && (
          <Badge variant="outline" className="border-warning/50 text-warning">
            <span className="size-1.5 rounded-full bg-warning" />
            {t(($) => $.captions.partial)}
          </Badge>
        )}
      </div>
      <p className="caption-source mt-2 max-w-[86ch] font-medium text-foreground">{segment.text}</p>
      <div className="mt-1.5 max-w-[86ch] border-l border-border pl-3">
        <TranslationBody segment={segment} />
      </div>
    </li>
  );
}
