import { Icon } from '@iconify/react';
import playIcon from '@iconify-icons/lucide/play';
import refreshIcon from '@iconify-icons/lucide/refresh-cw';
import squareIcon from '@iconify-icons/lucide/square';
import alertIcon from '@iconify-icons/lucide/circle-alert';
import dayjs from 'dayjs';
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
import type { EngineState } from '@/lib/engine';

const STATE_LABELS: Record<EngineState['state'], string> = {
  stopped: 'Stopped',
  starting: 'Starting',
  running: 'Running',
  stopping: 'Stopping',
  failed: 'Failed to start or run',
};

export function EngineStatus() {
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
  let actionError: Error | null = start.error ?? restart.error ?? stop.error;
  let stateError: string | undefined;
  if (state?.state === 'failed') stateError = state.error;
  let info: EngineInfo | undefined;
  if (state?.state === 'running' || state?.state === 'stopping') info = state.info;

  let statusLabel = 'Checking';
  if (state) statusLabel = STATE_LABELS[state.state];
  else if (isStatusError) statusLabel = 'Status query failed';

  const errorMessage = stateError ?? actionError?.message ?? statusError?.message;

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between gap-4">
          <CardTitle>Local engine</CardTitle>
          <span
            role="status"
            className="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground"
          >
            {statusLabel}
          </span>
        </div>
        <CardDescription>
          The desktop app manages the engine and exposes real runtime information over a loopback
          connection. Audio, recognition and translation are not connected yet.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-5">
        <div className="flex flex-wrap gap-3">
          <Button
            onClick={() => start.mutate()}
            disabled={isBusy || isStatusPending || state?.state === 'running'}
          >
            <Icon icon={playIcon} className="size-4" />
            Start
          </Button>
          <Button
            variant="outline"
            onClick={() => stop.mutate()}
            disabled={isBusy || state?.state !== 'running'}
          >
            <Icon icon={squareIcon} className="size-4" />
            Stop
          </Button>
          <Button
            variant="secondary"
            onClick={() => restart.mutate()}
            disabled={isBusy || isStatusPending || state?.state === 'stopped'}
          >
            <Icon icon={refreshIcon} className="size-4" />
            Restart
          </Button>
        </div>
        {(stateError || actionError || isStatusError) && (
          <div
            role="alert"
            className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive"
          >
            <Icon icon={alertIcon} className="mt-0.5 size-4 shrink-0" />
            <span>{errorMessage}</span>
          </div>
        )}
        {info && (
          <dl className="grid grid-cols-2 gap-x-6 gap-y-2 text-sm sm:grid-cols-3">
            <div>
              <dt className="text-muted-foreground">Engine version</dt>
              <dd className="font-mono">{info.engineVersion}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground">Node.js</dt>
              <dd className="font-mono">{info.nodeVersion}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground">Started at</dt>
              <dd>{dayjs(info.startedAt).format('HH:mm:ss')}</dd>
            </div>
          </dl>
        )}
        {import.meta.env.DEV && state?.state === 'running' && (
          <details className="rounded-md border border-border p-3 text-xs text-muted-foreground">
            <summary className="cursor-pointer">Development diagnostics</summary>
            <p className="mt-2 font-mono">
              PID {state.info.pid} · 127.0.0.1:{state.port}
            </p>
          </details>
        )}
      </CardContent>
    </Card>
  );
}
