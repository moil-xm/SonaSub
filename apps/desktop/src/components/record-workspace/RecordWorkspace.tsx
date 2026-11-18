import type { RecordDevice, RecordSnapshot } from '@/lib/records';
import type { Capabilities } from '@/lib/session';

import { RecordSubtitles } from './RecordSubtitles';
import { RecordToolbar } from './RecordToolbar';

type RecordWorkspaceProps = {
  capabilities: Capabilities;
  devices: RecordDevice[];
  snapshot: RecordSnapshot;
};

export function RecordWorkspace({ capabilities, devices, snapshot }: RecordWorkspaceProps) {
  return (
    <div className="mx-auto flex h-full w-full max-w-[1280px] flex-col gap-3 p-4 md:p-5">
      <RecordToolbar capabilities={capabilities} devices={devices} snapshot={snapshot} />
      <RecordSubtitles
        key={snapshot.summary.id}
        recordId={snapshot.summary.id}
        initialSegments={snapshot.segments}
        totalCount={snapshot.totalSubtitleCount}
        state={snapshot.summary.state}
        showSpeaker={snapshot.config.speakers.enabled}
      />
    </div>
  );
}
