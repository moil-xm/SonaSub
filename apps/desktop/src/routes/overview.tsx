import { LiveSubtitles } from '@/components/live-subtitles';
import { SessionToolbar } from '@/components/session-toolbar';

export function OverviewPage() {
  return (
    <div className="h-full overflow-hidden">
      <div className="mx-auto flex h-full w-full max-w-[1280px] flex-col gap-3 p-4 md:p-5">
        <SessionToolbar />
        <LiveSubtitles />
      </div>
    </div>
  );
}
