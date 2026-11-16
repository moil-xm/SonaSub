import { Icon } from '@iconify/react';
import alertCircleIcon from '@iconify-icons/lucide/alert-circle';
import infoIcon from '@iconify-icons/lucide/info';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import dayjs from 'dayjs';
import { useTranslation } from '@sonasub/i18n';
import { Card, CardContent, CardHeader, CardTitle } from '@sonasub/ui/components/card';
import { useQuery } from '@tanstack/react-query';

import { useNow } from '@/hooks/use-now';
import { formatDuration, formatLatency, formatPercent } from '@/lib/format';
import { sessionSnapshotOptions } from '@/lib/session';
import type { SessionNotice } from '@/lib/session.type';

type MetricProps = {
  label: string;
  value: string;
};

function Metric({ label, value }: MetricProps) {
  return (
    <div className="rounded-lg border bg-muted/20 px-3 py-2">
      <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
        {label}
      </p>
      <p className="mt-1 font-mono text-sm tabular-nums">{value}</p>
    </div>
  );
}

function NoticeIcon({ notice }: { notice: SessionNotice }) {
  if (notice.level === 'error')
    return <Icon icon={alertCircleIcon} className="size-4 text-destructive" />;
  if (notice.level === 'warning')
    return <Icon icon={triangleAlertIcon} className="size-4 text-warning" />;
  return <Icon icon={infoIcon} className="size-4 text-muted-foreground" />;
}

export function SessionDiagnostics() {
  const { t } = useTranslation();
  const now = useNow();
  const { data: snapshot } = useQuery(sessionSnapshotOptions);
  if (!snapshot) return null;

  const { stats, notices, status } = snapshot;
  let elapsed = 0;
  if (stats.startedAtMs > 0) elapsed = now - stats.startedAtMs;
  let audioLevel = Math.min(100, Math.round(stats.audioRms * 220));
  if (audioLevel < 0) audioLevel = 0;
  let runBoundary = '';
  if (status.state === 'running') {
    runBoundary = status.info.runBoundary;
  }
  const recentNotices = notices.toReversed().slice(0, 8);

  return (
    <div className="grid gap-5 xl:grid-cols-[minmax(0,1.35fr)_minmax(320px,0.65fr)]">
      <Card>
        <CardHeader>
          <CardTitle className="text-base">{t(($) => $.stats.title)}</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
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
          <div className="space-y-2">
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span>{t(($) => $.stats.audioLevel)}</span>
              <span className="font-mono">{audioLevel}%</span>
            </div>
            <div className="h-2 overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-success transition-[width] duration-150"
                style={{ width: `${audioLevel}%` }}
              />
            </div>
          </div>
          {runBoundary && (
            <p className="border-t pt-3 text-xs leading-5 text-muted-foreground">{runBoundary}</p>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-base">{t(($) => $.notices.title)}</CardTitle>
        </CardHeader>
        <CardContent>
          {recentNotices.length === 0 && (
            <p className="text-sm text-muted-foreground">{t(($) => $.notices.empty)}</p>
          )}
          {recentNotices.length > 0 && (
            <ul className="max-h-64 space-y-3 overflow-y-auto">
              {recentNotices.map((notice, index) => (
                <li
                  key={`${notice.atMs}-${index}`}
                  className="flex items-start gap-2 text-xs leading-5"
                >
                  <span className="mt-0.5 shrink-0">
                    <NoticeIcon notice={notice} />
                  </span>
                  <div className="min-w-0">
                    <time className="font-mono text-[11px] text-muted-foreground">
                      {dayjs(notice.atMs).format('HH:mm:ss')}
                    </time>
                    <p>{notice.message}</p>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
