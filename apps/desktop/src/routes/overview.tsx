import { useState } from 'react';

import { LiveSubtitles } from '@/components/live-subtitles';
import { SessionConfigForm } from '@/components/session-config-form';
import { SessionDiagnostics } from '@/components/session-diagnostics';
import { SessionToolbar } from '@/components/session-toolbar';

export function OverviewPage() {
  const [isConfigDirty, setIsConfigDirty] = useState(false);

  return (
    <div className="space-y-5">
      <SessionToolbar isConfigDirty={isConfigDirty} />
      <div className="grid items-start gap-5 xl:grid-cols-[minmax(0,1fr)_400px]">
        <LiveSubtitles />
        <SessionConfigForm onDirtyChange={setIsConfigDirty} />
      </div>
      <SessionDiagnostics />
    </div>
  );
}
