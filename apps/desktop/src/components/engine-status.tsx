import { Icon } from '@iconify/react';
import playIcon from '@iconify-icons/lucide/play';
import refreshIcon from '@iconify-icons/lucide/refresh-cw';
import squareIcon from '@iconify-icons/lucide/square';
import alertIcon from '@iconify-icons/lucide/circle-alert';
import dayjs from 'dayjs';
import { Button } from '@sonasub/ui/components/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@sonasub/ui/components/card';

import { useEngineAction, useEngineEvents, useEngineStatus, type EngineState } from '@/lib/engine';

const labels: Record<EngineState['state'], string> = {
  stopped: '已停止',
  starting: '正在启动',
  running: '运行中',
  stopping: '正在停止',
  failed: '启动或运行失败',
};

export function EngineStatus() {
  useEngineEvents();
  const status = useEngineStatus();
  const start = useEngineAction('start');
  const stop = useEngineAction('stop');
  const restart = useEngineAction('restart');
  const state = status.data;
  const busy =
    start.isPending ||
    stop.isPending ||
    restart.isPending ||
    state?.state === 'starting' ||
    state?.state === 'stopping';
  const error =
    state?.state === 'failed' ? state.error : (start.error ?? restart.error ?? stop.error);
  const info = state?.state === 'running' || state?.state === 'stopping' ? state.info : undefined;

  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between gap-4">
          <CardTitle>本地引擎</CardTitle>
          <span
            role="status"
            className="rounded-full border border-border px-3 py-1 text-xs font-medium text-muted-foreground"
          >
            {state ? labels[state.state] : status.isError ? '状态查询失败' : '正在查询'}
          </span>
        </div>
        <CardDescription>
          引擎由桌面应用管理，通过本机连接提供真实运行信息。音频、识别与翻译尚未接入。
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-5">
        <div className="flex flex-wrap gap-3">
          <Button
            onClick={() => start.mutate()}
            disabled={busy || status.isPending || state?.state === 'running'}
          >
            <Icon icon={playIcon} className="size-4" />
            启动
          </Button>
          <Button
            variant="outline"
            onClick={() => stop.mutate()}
            disabled={busy || state?.state !== 'running'}
          >
            <Icon icon={squareIcon} className="size-4" />
            停止
          </Button>
          <Button
            variant="secondary"
            onClick={() => restart.mutate()}
            disabled={busy || status.isPending || state?.state === 'stopped'}
          >
            <Icon icon={refreshIcon} className="size-4" />
            重启
          </Button>
        </div>
        {(error || status.isError) && (
          <div
            role="alert"
            className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive"
          >
            <Icon icon={alertIcon} className="mt-0.5 size-4 shrink-0" />
            <span>
              {error instanceof Error ? error.message : String(error ?? status.error?.message)}
            </span>
          </div>
        )}
        {info && (
          <dl className="grid grid-cols-2 gap-x-6 gap-y-2 text-sm sm:grid-cols-3">
            <div>
              <dt className="text-muted-foreground">引擎版本</dt>
              <dd className="font-mono">{info.engineVersion}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground">Node.js</dt>
              <dd className="font-mono">{info.nodeVersion}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground">启动时间</dt>
              <dd>{dayjs(info.startedAt).format('HH:mm:ss')}</dd>
            </div>
          </dl>
        )}
        {import.meta.env.DEV && state?.state === 'running' && (
          <details className="rounded-md border border-border p-3 text-xs text-muted-foreground">
            <summary className="cursor-pointer">开发诊断</summary>
            <p className="mt-2 font-mono">
              PID {state.info.pid} · 127.0.0.1:{state.port}
            </p>
          </details>
        )}
      </CardContent>
    </Card>
  );
}
