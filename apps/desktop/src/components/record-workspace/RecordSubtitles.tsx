import { useEffect, useRef, useState } from 'react';
import { Icon } from '@iconify/react';
import arrowUpIcon from '@iconify-icons/lucide/arrow-up';
import captionsIcon from '@iconify-icons/lucide/captions';
import downloadIcon from '@iconify-icons/lucide/download';
import loaderCircleIcon from '@iconify-icons/lucide/loader-circle';
import radioIcon from '@iconify-icons/lucide/radio';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import { useTranslation } from '@sonasub/i18n';
import { Alert, AlertDescription, AlertTitle } from '@sonasub/ui/components/alert';
import { Button } from '@sonasub/ui/components/button';
import { open } from '@tauri-apps/plugin-dialog';

import { SubtitleRow } from '@/components/live-subtitles';
import { getErrorMessage } from '@/lib/error';
import { isRecordActiveState, useExportRecord } from '@/lib/records';
import type { RecordExportSummary, RecordState } from '@/lib/records';
import type { SubtitleSegment } from '@/lib/session';

const FOLLOW_THRESHOLD_PX = 48;

type RecordSubtitlesProps = {
  initialSegments: SubtitleSegment[];
  recordId: string;
  showSpeaker: boolean;
  state: RecordState;
  totalCount: number;
};

type ExportStatus =
  | { kind: 'success'; summary: RecordExportSummary }
  | { kind: 'error'; message: string };

export function RecordSubtitles({
  initialSegments,
  recordId,
  showSpeaker,
  state,
  totalCount,
}: RecordSubtitlesProps) {
  const { t } = useTranslation();
  const viewportRef = useRef<HTMLDivElement>(null);
  const [isFollowing, setIsFollowing] = useState(true);
  const [exportStatus, setExportStatus] = useState<ExportStatus | null>(null);
  const exportRecord = useExportRecord();
  const segments = initialSegments;
  const isExportDisabled = isRecordActiveState(state) || state === 'notStarted';

  useEffect(() => {
    if (!isFollowing || segments.length === 0) return;
    const viewport = viewportRef.current;
    if (!viewport) return;
    viewport.scrollTop = viewport.scrollHeight;
  }, [segments, isFollowing]);

  function handleScroll() {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const distanceToEnd = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
    setIsFollowing(distanceToEnd < FOLLOW_THRESHOLD_PX);
  }

  function jumpToLatest() {
    const viewport = viewportRef.current;
    if (!viewport) return;
    setIsFollowing(true);
    viewport.scrollTop = viewport.scrollHeight;
  }

  async function handleExport() {
    setExportStatus(null);
    try {
      const destination = await open({
        directory: true,
        multiple: false,
        title: t(($) => $.records.workspace.exportDialogTitle),
      });
      if (!destination) return;
      const summary = await exportRecord.mutateAsync({ recordId, destination });
      setExportStatus({ kind: 'success', summary });
    } catch (error) {
      setExportStatus({ kind: 'error', message: getErrorMessage(error) });
    }
  }

  let emptyTitle: string = t(($) => $.records.workspace.emptyTitle);
  let emptyDescription: string = t(($) => $.records.workspace.emptyDescription);
  let emptyIcon = captionsIcon;
  if (state === 'running') {
    emptyTitle = t(($) => $.records.workspace.listeningTitle);
    emptyDescription = t(($) => $.records.workspace.listeningDescription);
    emptyIcon = radioIcon;
  }
  if (state === 'waitingForDevice') {
    emptyTitle = t(($) => $.records.status.waitingForDevice);
    emptyDescription = t(($) => $.records.workspace.waitingForDevice);
    emptyIcon = radioIcon;
  }
  if (state === 'failed') {
    emptyTitle = t(($) => $.records.status.failed);
    emptyDescription = t(($) => $.records.workspace.failed);
  }
  if (state === 'ended') {
    emptyTitle = t(($) => $.records.status.ended);
    emptyDescription = t(($) => $.records.workspace.ended);
  }

  let exportHint: string | undefined;
  if (state === 'notStarted') {
    exportHint = t(($) => $.records.workspace.exportDisabledEmpty);
  } else if (isRecordActiveState(state)) {
    exportHint = t(($) => $.records.workspace.exportDisabledActive);
  }

  return (
    <section className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border bg-card">
      <header className="flex h-[46px] shrink-0 items-center justify-between gap-3 border-b px-4">
        <div className="flex min-w-0 items-center gap-2">
          <Icon icon={captionsIcon} className="size-4 shrink-0 text-muted-foreground" />
          <h2 className="truncate text-[13px] font-medium">{t(($) => $.captions.title)}</h2>
          <span className="text-[11.5px] text-muted-foreground">
            {totalCount > segments.length
              ? t(($) => $.records.workspace.windowCount, {
                  shown: segments.length,
                  total: totalCount,
                })
              : t(($) => $.records.workspace.count, { count: totalCount })}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          {state === 'running' && (
            <span className="inline-flex items-center gap-1.5 rounded-full border border-success/40 px-2 py-0.5 text-[10.5px] font-medium text-success">
              <Icon icon={radioIcon} className="size-3 animate-pulse" />
              {t(($) => $.workbench.status.running)}
            </span>
          )}
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={isExportDisabled || exportRecord.isPending}
            title={exportHint}
            onClick={() => void handleExport()}
          >
            {exportRecord.isPending ? (
              <Icon icon={loaderCircleIcon} className="animate-spin" />
            ) : (
              <Icon icon={downloadIcon} />
            )}
            {t(($) => $.records.workspace.export)}
          </Button>
        </div>
      </header>

      {exportStatus && (
        <div className="shrink-0 border-b border-border/70 px-4 py-2">
          {exportStatus.kind === 'error' ? (
            <Alert variant="destructive">
              <Icon icon={triangleAlertIcon} />
              <AlertTitle>{t(($) => $.records.workspace.exportFailed)}</AlertTitle>
              <AlertDescription>{exportStatus.message}</AlertDescription>
            </Alert>
          ) : (
            <Alert>
              <Icon icon={downloadIcon} />
              <AlertTitle>{t(($) => $.records.workspace.exportSuccessTitle)}</AlertTitle>
              <AlertDescription>
                <span className="block break-all">
                  {t(($) => $.records.workspace.exportSuccess, {
                    directory: exportStatus.summary.directory,
                  })}
                </span>
                <span className="block">
                  {t(($) => $.records.workspace.exportCounts, {
                    audio: exportStatus.summary.audioFiles,
                    original: exportStatus.summary.originalLrcFiles,
                    translation: exportStatus.summary.translationLrcFiles,
                  })}
                </span>
                {exportStatus.summary.audioDisabled && (
                  <span className="block">{t(($) => $.records.workspace.exportAudioDisabled)}</span>
                )}
                {exportStatus.summary.missingAudioRuns > 0 && (
                  <span className="block">
                    {t(($) => $.records.workspace.exportMissingAudio, {
                      count: exportStatus.summary.missingAudioRuns,
                    })}
                  </span>
                )}
              </AlertDescription>
            </Alert>
          )}
        </div>
      )}

      <div className="relative min-h-0 flex-1">
        <div ref={viewportRef} className="h-full overflow-y-auto" onScroll={handleScroll}>
          {segments.length === 0 && (
            <div className="flex h-full min-h-[300px] flex-col items-center justify-center px-6 text-center">
              <div className="mb-3 grid size-10 place-items-center rounded-full bg-muted text-muted-foreground">
                <Icon icon={emptyIcon} className="size-4" />
              </div>
              <p className="text-[13px] font-medium">{emptyTitle}</p>
              <p className="mt-1.5 max-w-md text-xs leading-5 text-muted-foreground">
                {emptyDescription}
              </p>
            </div>
          )}

          {segments.length > 0 && (
            <ol aria-live="polite" className="divide-y divide-border/70">
              {segments.map((segment) => (
                <SubtitleRow key={segment.id} segment={segment} showSpeaker={showSpeaker} />
              ))}
            </ol>
          )}
        </div>
        {!isFollowing && segments.length > 0 && (
          <button
            type="button"
            className="absolute right-4 bottom-4 inline-flex items-center gap-1.5 rounded-full border bg-popover px-3 py-1.5 text-xs font-medium text-foreground shadow-md transition-colors hover:bg-muted"
            onClick={jumpToLatest}
          >
            <Icon icon={arrowUpIcon} className="size-3.5" />
            {t(($) => $.captions.jumpToLatest)}
          </button>
        )}
      </div>
    </section>
  );
}
