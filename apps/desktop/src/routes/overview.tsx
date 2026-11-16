import { EngineStatus } from '@/components/engine-status';

export function OverviewPage() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-xl font-semibold">Runtime overview</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Manage local engine startup, connection and shutdown.
        </p>
      </div>
      <EngineStatus />
    </div>
  );
}
