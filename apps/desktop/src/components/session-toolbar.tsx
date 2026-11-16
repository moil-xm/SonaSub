import { Icon } from '@iconify/react';
import loaderCircleIcon from '@iconify-icons/lucide/loader-circle';
import playIcon from '@iconify-icons/lucide/play';
import rotateCwIcon from '@iconify-icons/lucide/rotate-cw';
import squareIcon from '@iconify-icons/lucide/square';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import { useTranslation } from '@sonasub/i18n';
import { Alert, AlertDescription, AlertTitle } from '@sonasub/ui/components/alert';
import { Button } from '@sonasub/ui/components/button';
import { Card, CardContent } from '@sonasub/ui/components/card';
import { useQuery } from '@tanstack/react-query';

import { useNow } from '@/hooks/use-now';
import { getErrorMessage } from '@/lib/error';
import { formatDuration, formatPercent } from '@/lib/format';
import { assessModelReadiness } from '@/lib/model-readiness';
import {
  sessionCapabilitiesOptions,
  sessionConfigOptions,
  sessionSnapshotOptions,
  useSessionAction,
} from '@/lib/session';
import type { SessionStatus } from '@/lib/session.type';

import { ReadinessIssues } from './readiness-issues';

type SessionToolbarProps = {
  isConfigDirty: boolean;
};

function statusTone(status: SessionStatus): string {
  if (status.state === 'running') return 'bg-success';
  if (status.state === 'starting' || status.state === 'stopping') return 'bg-warning animate-pulse';
  if (status.state === 'failed') return 'bg-destructive';
  return 'bg-muted-foreground';
}

type ActionIconProps = {
  icon: typeof playIcon;
  isLoading: boolean;
};

function ActionIcon({ icon, isLoading }: ActionIconProps) {
  if (isLoading) return <Icon icon={loaderCircleIcon} className="animate-spin" />;
  return <Icon icon={icon} />;
}

export function SessionToolbar({ isConfigDirty }: SessionToolbarProps) {
  const { t } = useTranslation();
  const now = useNow();
  const { data: capabilities, error: capabilitiesError } = useQuery(sessionCapabilitiesOptions);
  const { data: configView, error: configError } = useQuery(sessionConfigOptions);
  const { data: snapshot, error: snapshotError } = useQuery(sessionSnapshotOptions);
  const action = useSessionAction();

  if (!snapshot || !configView || !capabilities) {
    return (
      <Card>
        <CardContent className="flex min-h-36 items-center justify-center text-sm text-muted-foreground">
          {t(($) => $.workbench.status.loading)}
        </CardContent>
      </Card>
    );
  }

  const { status } = snapshot;
  const readiness = assessModelReadiness(capabilities, configView.config);
  const isStarting = status.state === 'starting';
  const isRunning = status.state === 'running';
  const isStopping = status.state === 'stopping';
  const isBusy = isStarting || isStopping || action.isPending;
  let statusLabel: string = t(($) => $.workbench.status.idle);
  let statusDetail: string = t(($) => $.workbench.status.idleDetail);
  if (isStarting) {
    statusLabel = t(($) => $.workbench.status.starting);
    statusDetail = status.stage;
  }
  if (isRunning) {
    statusLabel = t(($) => $.workbench.status.running);
    statusDetail = t(($) => $.workbench.status.runningDetail, {
      duration: formatDuration(now - status.info.startedAtMs),
    });
  }
  if (isStopping) {
    statusLabel = t(($) => $.workbench.status.stopping);
    statusDetail = status.stage;
  }
  if (status.state === 'failed') {
    statusLabel = t(($) => $.workbench.status.failed);
    statusDetail = status.error;
  }

  let asrRoute: string = t(($) => $.workbench.route.asrAuto);
  if (readiness.selectedAsr?.kind === 'streaming') {
    asrRoute = t(($) => $.workbench.route.asrStreaming);
  }
  if (readiness.selectedAsr?.kind === 'segmented') {
    asrRoute = t(($) => $.workbench.route.asrSegmented);
  }
  if (isRunning) {
    if (status.info.asr.mode === 'streaming') {
      asrRoute = t(($) => $.workbench.route.asrStreaming);
    } else {
      asrRoute = t(($) => $.workbench.route.asrSegmented);
    }
  }

  let translationRoute: string = t(($) => $.workbench.route.translationText);
  if (isRunning) translationRoute = status.info.translation.modelLabel;

  let speakerRoute: string = t(($) => $.workbench.route.speakersOff);
  if (configView.config.speakers.enabled) {
    speakerRoute = t(($) => $.workbench.route.speakersOn);
  }
  if (isRunning && status.info.speakers.enabled) {
    if (status.info.speakers.diarization) {
      speakerRoute = t(($) => $.workbench.route.speakersDiarization);
    } else {
      speakerRoute = t(($) => $.workbench.route.speakersEmbedding);
    }
  }

  let startHint: string | undefined;
  if (isConfigDirty) startHint = t(($) => $.workbench.dirty);
  else if (!readiness.canStart) startHint = t(($) => $.workbench.readiness.title);

  return (
    <div className="space-y-4">
      <Card className="overflow-hidden">
        <CardContent className="space-y-5 p-5 lg:p-6">
          <div className="flex flex-col gap-5 xl:flex-row xl:items-start xl:justify-between">
            <div className="min-w-0 space-y-2">
              <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
                <span className="inline-flex items-center gap-2">
                  <span className={`size-2 rounded-full ${statusTone(status)}`} />
                  <span role="status">{statusLabel}</span>
                </span>
                <span className="h-3 w-px bg-border" />
                <span>{t(($) => $.workbench.eyebrow)}</span>
              </div>
              <h1 className="max-w-3xl text-2xl font-semibold tracking-tight">
                {t(($) => $.workbench.title)}
              </h1>
              <p className="max-w-3xl text-sm leading-6 text-muted-foreground">
                {t(($) => $.workbench.description)}
              </p>
              <p className="max-w-3xl text-xs text-muted-foreground">{statusDetail}</p>
            </div>

            <div className="flex shrink-0 flex-wrap items-center gap-2">
              <Button
                disabled={isBusy || isRunning || isConfigDirty || !readiness.canStart}
                title={startHint}
                onClick={() => action.mutate('start')}
              >
                <ActionIcon
                  icon={playIcon}
                  isLoading={action.isPending && action.variables === 'start'}
                />
                {t(($) => $.workbench.actions.start)}
              </Button>
              <Button
                variant="outline"
                disabled={isBusy || (!isRunning && !isStarting)}
                onClick={() => action.mutate('stop')}
              >
                <ActionIcon
                  icon={squareIcon}
                  isLoading={action.isPending && action.variables === 'stop'}
                />
                {t(($) => $.workbench.actions.stop)}
              </Button>
              <Button
                variant="outline"
                disabled={isBusy || isConfigDirty || !readiness.canStart}
                title={startHint}
                onClick={() => action.mutate('restart')}
              >
                <ActionIcon
                  icon={rotateCwIcon}
                  isLoading={action.isPending && action.variables === 'restart'}
                />
                {t(($) => $.workbench.actions.restart)}
              </Button>
            </div>
          </div>

          <div className="grid gap-px overflow-hidden rounded-lg border bg-border sm:grid-cols-2 xl:grid-cols-4">
            <div className="bg-card p-3">
              <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                {t(($) => $.workbench.route.asr)}
              </p>
              <p className="mt-1 text-sm font-medium">{asrRoute}</p>
            </div>
            <div className="bg-card p-3">
              <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                {t(($) => $.workbench.route.translation)}
              </p>
              <p className="mt-1 text-sm font-medium">{translationRoute}</p>
            </div>
            <div className="bg-card p-3">
              <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                {t(($) => $.workbench.route.speakers)}
              </p>
              <p className="mt-1 text-sm font-medium">{speakerRoute}</p>
            </div>
            <div className="bg-card p-3">
              <p className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                {t(($) => $.workbench.route.resources)}
              </p>
              <p className="mt-1 text-sm font-medium">
                {t(($) => $.workbench.route.resourceValue, {
                  cpu: formatPercent(snapshot.stats.cpuPercent),
                  memory: snapshot.stats.rssMb.toFixed(0),
                })}
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {status.state === 'failed' && (
        <Alert variant="destructive">
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.workbench.status.failed)}</AlertTitle>
          <AlertDescription>{status.error}</AlertDescription>
        </Alert>
      )}

      {!readiness.canStart && !isRunning && !isStarting && (
        <Alert>
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.workbench.readiness.title)}</AlertTitle>
          <AlertDescription>
            <ReadinessIssues issues={readiness.issues} />
          </AlertDescription>
        </Alert>
      )}

      {action.error && (
        <Alert variant="destructive">
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.workbench.errors.action)}</AlertTitle>
          <AlertDescription>{getErrorMessage(action.error)}</AlertDescription>
        </Alert>
      )}

      {capabilitiesError && (
        <Alert variant="destructive">
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.workbench.errors.capabilities)}</AlertTitle>
          <AlertDescription>{getErrorMessage(capabilitiesError)}</AlertDescription>
        </Alert>
      )}

      {configError && (
        <Alert variant="destructive">
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.workbench.errors.config)}</AlertTitle>
          <AlertDescription>{getErrorMessage(configError)}</AlertDescription>
        </Alert>
      )}

      {snapshotError && (
        <Alert variant="destructive">
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.workbench.errors.snapshot)}</AlertTitle>
          <AlertDescription>{getErrorMessage(snapshotError)}</AlertDescription>
        </Alert>
      )}
    </div>
  );
}
