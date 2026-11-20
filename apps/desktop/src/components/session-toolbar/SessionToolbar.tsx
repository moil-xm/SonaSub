import { Icon } from '@iconify/react';
import playIcon from '@iconify-icons/lucide/play';
import rotateCwIcon from '@iconify-icons/lucide/rotate-cw';
import settingsIcon from '@iconify-icons/lucide/settings';
import squareIcon from '@iconify-icons/lucide/square';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import { useTranslation } from '@sonasub/i18n';
import { Alert, AlertDescription, AlertTitle } from '@sonasub/ui/components/alert';
import { Button } from '@sonasub/ui/components/button';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';

import { ReadinessIssues } from '@/components/readiness-issues';
import { useNow } from '@/hooks/use-now';
import { getErrorMessage } from '@/lib/error';
import { formatDuration } from '@/lib/format';
import { assessModelReadiness } from '@/lib/model-readiness';
import {
  audioDeviceKey,
  sessionCapabilitiesOptions,
  sessionConfigOptions,
  sessionSnapshotOptions,
  useSessionAction,
} from '@/lib/session';
import type { SessionStatus } from '@/lib/session';

import { ActionIcon } from './ActionIcon';
import { SessionDataErrors } from './SessionDataErrors';
import { SessionMetric } from './SessionMetric';

function statusTone(status: SessionStatus): string {
  if (status.state === 'running') return 'bg-success';
  if (status.state === 'starting' || status.state === 'stopping') {
    return 'bg-warning animate-pulse';
  }
  if (status.state === 'failed') return 'bg-destructive';
  return 'bg-muted-foreground';
}

export function SessionToolbar() {
  const { t } = useTranslation();
  const now = useNow();
  const { data: capabilities, error: capabilitiesError } = useQuery(sessionCapabilitiesOptions);
  const { data: configView, error: configError } = useQuery(sessionConfigOptions);
  const { data: snapshot, error: snapshotError } = useQuery(sessionSnapshotOptions);
  const action = useSessionAction();

  const isInitialLoading =
    (!snapshot && !snapshotError) ||
    (!configView && !configError) ||
    (!capabilities && !capabilitiesError);

  if (isInitialLoading) {
    return (
      <section className="flex min-h-[148px] items-center justify-center rounded-xl border bg-card px-5 py-4 text-sm text-muted-foreground">
        {t(($) => $.workbench.status.loading)}
      </section>
    );
  }

  if (!snapshot || !configView || !capabilities) {
    return (
      <div className="shrink-0 space-y-2.5">
        <section className="rounded-xl border bg-card px-5 py-5">
          <h1 className="text-xl font-semibold tracking-tight">{t(($) => $.workbench.title)}</h1>
          <p className="mt-1 max-w-3xl text-xs leading-5 text-muted-foreground">
            {t(($) => $.workbench.description)}
          </p>
        </section>
        <SessionDataErrors
          capabilitiesError={capabilitiesError}
          configError={configError}
          snapshotError={snapshotError}
        />
      </div>
    );
  }

  const { status } = snapshot;
  const readiness = assessModelReadiness(capabilities, configView.config);
  const selectedTranslationEngine = capabilities.translationEngines.find(
    (engine) => engine.id === configView.config.translation.engine,
  );
  const selectedTranslation = capabilities.translationModels.find(
    (model) =>
      model.engine === configView.config.translation.engine &&
      model.id === configView.config.translation.modelId,
  );
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

  let translationRoute: string =
    selectedTranslation?.label ?? configView.config.translation.modelId;
  if (selectedTranslationEngine?.id === 'openai-compatible') {
    const model = configView.config.translation.openaiCompatible.model.trim();
    translationRoute = model
      ? `${selectedTranslationEngine.label} · ${model}`
      : selectedTranslationEngine.label;
  }
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

  const liveSourceKind = configView.config.audio.live.kind;
  let sourceLabel: string = t(($) => $.config.systemOutput);
  if (liveSourceKind === 'microphone') sourceLabel = t(($) => $.config.microphone);
  let deviceRef = configView.config.audio.live.deviceId
    ? {
        kind: liveSourceKind,
        id: configView.config.audio.live.deviceId,
      }
    : null;
  if (!deviceRef && liveSourceKind === 'systemOutput') {
    deviceRef = capabilities.audio.defaultSystemOutput;
  }
  if (!deviceRef && liveSourceKind === 'microphone') {
    deviceRef = capabilities.audio.defaultMicrophone;
  }
  const selectedDevice = deviceRef
    ? capabilities.audio.devices.find(
        (device) => audioDeviceKey(device) === audioDeviceKey(deviceRef),
      )
    : undefined;
  let deviceLabel: string = t(($) => $.config.noAudioDevice);
  if (selectedDevice) deviceLabel = selectedDevice.name;
  let audioRoute = `${sourceLabel} · ${deviceLabel}`;
  if (isRunning) {
    let runningSourceLabel: string = t(($) => $.config.systemOutput);
    if (status.info.audio.sourceKind === 'microphone') {
      runningSourceLabel = t(($) => $.config.microphone);
    }
    audioRoute = `${runningSourceLabel} · ${status.info.audio.deviceName}`;
  }

  let startHint: string | undefined;
  if (!readiness.canStart) startHint = t(($) => $.workbench.readiness.title);

  let readinessLabel: string = t(($) => $.workbench.readiness.setupRequired);
  if (readiness.canStart) readinessLabel = t(($) => $.workbench.readiness.ready);
  let readinessTone = 'text-muted-foreground';
  if (readiness.canStart) readinessTone = 'text-success';
  else readinessTone = 'text-warning';

  return (
    <div className="shrink-0 space-y-2.5">
      <section className="overflow-hidden rounded-xl border bg-card">
        <div className="flex flex-col gap-4 px-4 py-4 min-[1120px]:flex-row min-[1120px]:items-start min-[1120px]:justify-between md:px-5">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1 text-[11px] font-medium tracking-[0.05em] text-muted-foreground uppercase">
              <span className="inline-flex items-center gap-1.5">
                <span className={`size-2 rounded-full ${statusTone(status)}`} />
                <span role="status">{statusLabel}</span>
              </span>
              <span className="h-3 w-px bg-border" />
              <span>{t(($) => $.workbench.eyebrow)}</span>
              {!isRunning && (
                <>
                  <span className="h-3 w-px bg-border" />
                  <span className={readinessTone}>{readinessLabel}</span>
                </>
              )}
            </div>
            <h1 className="mt-1.5 text-xl font-semibold tracking-tight">
              {t(($) => $.workbench.title)}
            </h1>
            <p className="mt-1 max-w-3xl text-xs leading-5 text-muted-foreground">{statusDetail}</p>
          </div>

          <div className="flex shrink-0 flex-wrap items-center gap-2">
            <Button
              size="sm"
              disabled={isBusy || isRunning || !readiness.canStart}
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
              size="sm"
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
              size="sm"
              disabled={isBusy || !readiness.canStart}
              title={startHint}
              onClick={() => action.mutate('restart')}
            >
              <ActionIcon
                icon={rotateCwIcon}
                isLoading={action.isPending && action.variables === 'restart'}
              />
              {t(($) => $.workbench.actions.restart)}
            </Button>
            <Button variant="ghost" size="sm" className="text-muted-foreground" asChild>
              <Link to="/session">
                <Icon icon={settingsIcon} />
                {t(($) => $.workbench.actions.settings)}
              </Link>
            </Button>
          </div>
        </div>

        <dl className="grid gap-px border-t bg-border/70 sm:grid-cols-2 min-[1120px]:grid-cols-4">
          <SessionMetric label={t(($) => $.workbench.route.audio)} value={audioRoute} />
          <SessionMetric label={t(($) => $.workbench.route.asr)} value={asrRoute} />
          <SessionMetric label={t(($) => $.workbench.route.translation)} value={translationRoute} />
          <SessionMetric label={t(($) => $.workbench.route.speakers)} value={speakerRoute} />
        </dl>
      </section>

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

      <SessionDataErrors
        capabilitiesError={capabilitiesError}
        configError={configError}
        snapshotError={snapshotError}
      />
    </div>
  );
}
