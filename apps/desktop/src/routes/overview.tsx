import { EngineStatus } from '@/components/engine-status';

export function OverviewPage() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-xl font-semibold">运行概览</h1>
        <p className="mt-1 text-sm text-muted-foreground">管理本地引擎的启动、连接与退出。</p>
      </div>
      <EngineStatus />
    </div>
  );
}
