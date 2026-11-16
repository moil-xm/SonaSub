import { Icon } from '@iconify/react';
import playIcon from '@iconify-icons/lucide/play';
import refreshIcon from '@iconify-icons/lucide/refresh-cw';
import squareIcon from '@iconify-icons/lucide/square';
import alertIcon from '@iconify-icons/lucide/circle-alert';
import dayjs from 'dayjs';
import { useTranslation } from '@sonasub/i18n';
import { useQuery } from '@tanstack/react-query';
import { Button } from '@sonasub/ui/components/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@sonasub/ui/components/card';
import type { EngineInfo } from '@sonasub/protocol/engine';

import { engineStatusOptions, useEngineAction, useEngineEvents } from '@/lib/engine';

export function EngineStatus() {
  const { t } = useTranslation();
  useEngineEvents();
  const {
    data: state,
    isPending: isStatusPending,
    isError: isStatusError,
    error: statusError,
  } = useQuery(engineStatusOptions);
  const start = useEngineAction('start');
  const stop = useEngineAction('stop');
  const restart = useEngineAction('restart');
  const isBusy =
    start.isPending ||
    stop.isPending ||
    restart.isPending ||
    state?.state === 'starting' ||
    state?.state === 'stopping';
  const isEngineFailed = state?.state === 'failed';
  let actionError: Error | null = start.error ?? restart.error ?? stop.error;
  let stateError: string | undefined;
  if (isEngineFailed) stateError = state.error;
  let info: EngineInfo | undefined;
  if (state?.state === 'running' || state?.state === 'stopping') info = state.info;

  let statusLabel: string = t(($) => $.engine.status.checking);
  if (state) {
    switch (state.state) {
      case 'stopped':
        statusLabel = t(($) => $.engine.states.stopped);
        break;
      case 'starting':
        statusLabel = t(($) => $.engine.states.starting);
        break;
      case 'running':
        statusLabel = t(($) => $.engine.states.running);
        break;
      case 'stopping':
        statusLabel = t(($) => $.engine.states.stopping);
        break;
      case 'failed':
        statusLabel = t(($) => $.engine.states.failed);
        break;
    }
  } else if (isStatusError) statusLabel = t(($) => $.engine.status.queryFailed);

  let errorTitle: string = t(($) => $.engine.errors.unknown);
  if (isEngineFailed) errorTitle = t(($) => $.engine.errors.failed);
  else if (actionError) errorTitle = t(($) => $.engine.errors.operationFailed);
  else if (isStatusError) errorTitle = t(($) => $.engine.errors.statusQueryFailed);

  let errorMessage: string = t(($) => $.engine.errors.unknown);
  if (stateError) errorMessage = stateError;
  else if (actionError?.message) errorMessage = actionError.message;
  else if (statusError?.message) errorMessage = statusError.message;

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between gap-4">
          <CardTitle>{t(($) => $.engine.title)}</CardTitle>
          <span
            role="status"
            className="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground"
          >
            {statusLabel}
          </span>
        </div>
        <CardDescription>{t(($) => $.engine.description)}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-5">
        <div className="flex flex-wrap gap-3">
          <Button
            onClick={() => start.mutate()}
            disabled={isBusy || isStatusPending || state?.state === 'running'}
          >
            <Icon icon={playIcon} className="size-4" />
            {t(($) => $.engine.actions.start)}
          </Button>
          <Button
            variant="outline"
            onClick={() => stop.mutate()}
            disabled={isBusy || state?.state !== 'running'}
          >
            <Icon icon={squareIcon} className="size-4" />
            {t(($) => $.engine.actions.stop)}
          </Button>
          <Button
            variant="secondary"
            onClick={() => restart.mutate()}
            disabled={isBusy || isStatusPending || state?.state === 'stopped'}
          >
            <Icon icon={refreshIcon} className="size-4" />
            {t(($) => $.engine.actions.restart)}
          </Button>
        </div>
        {(isEngineFailed || actionError || isStatusError) && (
          <div
            role="alert"
            className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive"
          >
            <Icon icon={alertIcon} className="mt-0.5 size-4 shrink-0" />
            <div className="min-w-0 space-y-1">
              <p className="font-medium">{errorTitle}</p>
              <p className="break-words font-mono text-xs whitespace-pre-wrap">{errorMessage}</p>
            </div>
          </div>
        )}
        {info && (
          <dl className="grid grid-cols-2 gap-x-6 gap-y-2 text-sm sm:grid-cols-3">
            <div>
              <dt className="text-muted-foreground">{t(($) => $.engine.details.version)}</dt>
              <dd className="font-mono">{info.engineVersion}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground">Node.js</dt>
              <dd className="font-mono">{info.nodeVersion}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground">{t(($) => $.engine.details.startedAt)}</dt>
              <dd>{dayjs(info.startedAt).format('HH:mm:ss')}</dd>
            </div>
          </dl>
        )}
        {import.meta.env.DEV && state?.state === 'running' && (
          <details className="rounded-md border border-border p-3 text-xs text-muted-foreground">
            <summary className="cursor-pointer">{t(($) => $.engine.diagnostics.title)}</summary>
            <p className="mt-2 font-mono">
              PID {state.info.pid} · 127.0.0.1:{state.port}
            </p>
          </details>
        )}
      </CardContent>
    </Card>
  );
}
