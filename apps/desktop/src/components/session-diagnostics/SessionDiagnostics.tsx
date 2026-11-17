import { Icon } from '@iconify/react';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import dayjs from 'dayjs';
import { useTranslation } from '@sonasub/i18n';
import { Alert, AlertDescription, AlertTitle } from '@sonasub/ui/components/alert';
import { useQuery } from '@tanstack/react-query';

import { useNow } from '@/hooks/use-now';
import { getErrorMessage } from '@/lib/error';
import { formatDuration, formatLatency, formatPercent } from '@/lib/format';
import { sessionSnapshotOptions } from '@/lib/session';
import { SettingsList, SettingsSection } from '@/components/settings-section';

import { Metric } from './Metric';
import { NoticeIcon } from './NoticeIcon';

export function SessionDiagnostics() {
  const { t } = useTranslation();
  const now = useNow();
  const { data: snapshot, isPending, error } = useQuery(sessionSnapshotOptions);

  if (isPending) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        {t(($) => $.diagnostics.loading)}
      </div>
    );
  }

  if (!snapshot) {
    return (
      <div className="h-full overflow-y-auto">
        <div className="mx-auto w-full max-w-[1040px] px-4 py-4 md:px-6 md:py-5">
          <Alert variant="destructive">
            <Icon icon={triangleAlertIcon} />
            <AlertTitle>{t(($) => $.workbench.errors.snapshot)}</AlertTitle>
            <AlertDescription>{getErrorMessage(error)}</AlertDescription>
          </Alert>
        </div>
      </div>
    );
  }

  const { stats, notices, status } = snapshot;
  let elapsed = 0;
  if (stats.startedAtMs > 0) elapsed = now - stats.startedAtMs;
  let audioLevel = Math.min(100, Math.round(stats.audioRms * 220));
  if (audioLevel < 0) audioLevel = 0;
  let runBoundary = '';
  if (status.state === 'running') runBoundary = status.info.runBoundary;
  const recentNotices = notices.toReversed().slice(0, 20);

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto w-full max-w-[1040px] space-y-6 px-4 py-4 md:px-6 md:py-5">
        <p className="max-w-2xl text-xs leading-5 text-muted-foreground">
          {t(($) => $.diagnostics.description)}
        </p>

        <SettingsSection title={t(($) => $.stats.title)}>
          <SettingsList>
            <div className="grid grid-cols-2 gap-px bg-border/70 md:grid-cols-4">
              <Metric label={t(($) => $.stats.elapsed)} value={formatDuration(elapsed)} />
              <Metric label={t(($) => $.stats.cpu)} value={formatPercent(stats.cpuPercent)} />
              <Metric label={t(($) => $.stats.memory)} value={`${stats.rssMb.toFixed(0)} MB`} />
              <Metric label={t(($) => $.stats.captions)} value={String(stats.finalizedSubtitles)} />
              <Metric
                label={t(($) => $.stats.asrLatency)}
                value={formatLatency(stats.averageSubtitleLatencyMs)}
              />
              <Metric
                label={t(($) => $.stats.translationLatency)}
                value={formatLatency(stats.averageTranslationLatencyMs)}
              />
              <Metric
                label={t(($) => $.stats.speakerLatency)}
                value={formatLatency(stats.lastSpeakerLatencyMs)}
              />
              <Metric label={t(($) => $.stats.dropped)} value={String(stats.droppedFrames)} />
            </div>
            <div className="border-t border-border/70 px-4 py-3">
              <div className="flex items-center justify-between text-xs text-muted-foreground">
                <span>{t(($) => $.stats.audioLevel)}</span>
                <span className="font-mono tabular-nums">{audioLevel}%</span>
              </div>
              <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-muted">
                <div
                  className="h-full rounded-full bg-success transition-[width] duration-150"
                  style={{ width: `${audioLevel}%` }}
                />
              </div>
            </div>
            {runBoundary && (
              <div className="border-t border-border/70 px-4 py-3 text-xs leading-5 text-muted-foreground">
                {runBoundary}
              </div>
            )}
          </SettingsList>
        </SettingsSection>

        <SettingsSection title={t(($) => $.notices.title)}>
          <SettingsList>
            {recentNotices.length === 0 && (
              <p className="px-4 py-5 text-xs text-muted-foreground">{t(($) => $.notices.empty)}</p>
            )}
            {recentNotices.length > 0 && (
              <ul className="divide-y divide-border/70">
                {recentNotices.map((notice, index) => (
                  <li
                    key={`${notice.atMs}-${index}`}
                    className="flex items-start gap-2.5 px-4 py-3 text-xs leading-5"
                  >
                    <span className="mt-0.5 shrink-0">
                      <NoticeIcon notice={notice} />
                    </span>
                    <time className="shrink-0 font-mono text-[11px] text-muted-foreground">
                      {dayjs(notice.atMs).format('HH:mm:ss')}
                    </time>
                    <p className="min-w-0">{notice.message}</p>
                  </li>
                ))}
              </ul>
            )}
          </SettingsList>
        </SettingsSection>
      </div>
    </div>
  );
}
