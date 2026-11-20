import { Icon } from '@iconify/react';
import copyIcon from '@iconify-icons/lucide/copy';
import loaderCircleIcon from '@iconify-icons/lucide/loader-circle';
import pauseIcon from '@iconify-icons/lucide/pause';
import pencilIcon from '@iconify-icons/lucide/pencil';
import playIcon from '@iconify-icons/lucide/play';
import squareIcon from '@iconify-icons/lucide/square';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import { useTranslation } from '@sonasub/i18n';
import { Alert, AlertDescription, AlertTitle } from '@sonasub/ui/components/alert';
import { Button } from '@sonasub/ui/components/button';
import { Link, useNavigate } from '@tanstack/react-router';

import { RecordDevicePicker } from '@/components/record-device-picker';
import { RecordStateDot } from '@/components/record-nav';
import { LanguageName } from '@/components/session-config-form';
import { getErrorMessage } from '@/lib/error';
import {
  isRecordActiveState,
  useAddRecordDevice,
  useCreateRecord,
  useRecordAction,
  useRemoveRecordDevice,
} from '@/lib/records';
import type { RecordDevice, RecordSnapshot } from '@/lib/records';
import type { Capabilities } from '@/lib/session';

import { RecordMetric } from './RecordMetric';

type RecordToolbarProps = {
  capabilities: Capabilities;
  devices: RecordDevice[];
  snapshot: RecordSnapshot;
};

export function RecordToolbar({ capabilities, devices, snapshot }: RecordToolbarProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const action = useRecordAction();
  const addDevice = useAddRecordDevice();
  const removeDevice = useRemoveRecordDevice();
  const createRecord = useCreateRecord();
  const { summary, config } = snapshot;
  const isActive = isRecordActiveState(summary.state);
  const isStarting = summary.state === 'starting';
  const isRunning = summary.state === 'running';
  const isPaused = summary.state === 'paused';
  const isEnded = summary.state === 'ended';
  const selectedAsr = capabilities.asrModels.find(
    (model) => model.id === config.recognition.modelId,
  );
  const selectedTranslationEngine = capabilities.translationEngines.find(
    (engine) => engine.id === config.translation.engine,
  );
  const selectedTranslation = capabilities.translationModels.find(
    (model) =>
      model.engine === config.translation.engine && model.id === config.translation.modelId,
  );
  let translationLabel = selectedTranslation?.label ?? config.translation.modelId;
  if (selectedTranslationEngine?.id === 'openai-compatible') {
    const model = config.translation.openaiCompatible.model.trim();
    translationLabel = model
      ? `${selectedTranslationEngine.label} · ${model}`
      : selectedTranslationEngine.label;
  }
  const isActionPending = action.isPending || addDevice.isPending || removeDevice.isPending;
  const actionError = action.error ?? addDevice.error ?? removeDevice.error;
  let shouldShowActionError = Boolean(actionError);
  if (
    actionError &&
    summary.state === 'failed' &&
    summary.lastError &&
    getErrorMessage(actionError) === summary.lastError
  ) {
    shouldShowActionError = false;
  }
  let startAction: 'start' | 'resume' = 'start';
  if (isPaused) startAction = 'resume';
  let startLabel: string = t(($) => $.records.workspace.start);
  if (isPaused) startLabel = t(($) => $.records.workspace.resume);
  const isStartPending = action.isPending && action.variables?.action === startAction;
  const isPausePending = action.isPending && action.variables?.action === 'pause';
  const isStopPending = action.isPending && action.variables?.action === 'stop';
  let speakerLabel: string = t(($) => $.records.workspace.speakersOff);
  if (config.speakers.enabled) speakerLabel = t(($) => $.records.workspace.speakersOn);

  async function handleCopy() {
    const copied = await createRecord.mutateAsync({
      name: summary.name,
      config: snapshot.config,
    });
    await navigate({ to: '/', search: { record: copied.summary.id } });
  }

  return (
    <div className="shrink-0 space-y-2.5">
      <section className="overflow-hidden rounded-xl border bg-card">
        <div className="flex flex-col gap-4 px-4 py-4 md:px-5 min-[1060px]:flex-row min-[1060px]:items-start min-[1060px]:justify-between">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1 text-[11px] font-medium text-muted-foreground">
              <span className="inline-flex items-center gap-1.5">
                <RecordStateDot
                  state={summary.state}
                  hasActiveSource={summary.hasActiveSource}
                  label={t(($) => $.records.status[summary.state])}
                />
                <span role="status">{t(($) => $.records.status[summary.state])}</span>
              </span>
              <span className="h-3 w-px bg-border" />
              <span>
                {t(($) => $.records.nav.activeDeviceCount, {
                  count: summary.activeDevices.length,
                })}
              </span>
            </div>
            <h1 className="mt-1.5 truncate text-xl font-semibold tracking-tight">{summary.name}</h1>
            {summary.lastError && summary.state !== 'failed' && (
              <p className="mt-1 max-w-3xl text-xs leading-5 text-destructive">
                {summary.lastError}
              </p>
            )}
          </div>

          <div className="flex shrink-0 flex-wrap items-center gap-2">
            {!isActive && !isEnded && (
              <Button
                size="sm"
                disabled={isActionPending}
                onClick={() => action.mutate({ recordId: summary.id, action: startAction })}
              >
                {isStartPending && <Icon icon={loaderCircleIcon} className="animate-spin" />}
                {!isStartPending && <Icon icon={playIcon} />}
                {startLabel}
              </Button>
            )}
            {(isRunning || summary.state === 'waitingForDevice') && (
              <Button
                variant="outline"
                size="sm"
                disabled={isActionPending}
                onClick={() => action.mutate({ recordId: summary.id, action: 'pause' })}
              >
                {isPausePending && <Icon icon={loaderCircleIcon} className="animate-spin" />}
                {!isPausePending && <Icon icon={pauseIcon} />}
                {t(($) => $.records.workspace.pause)}
              </Button>
            )}
            {isActive && (
              <Button
                variant="outline"
                size="sm"
                disabled={isStarting || action.isPending}
                onClick={() => action.mutate({ recordId: summary.id, action: 'stop' })}
              >
                {isStopPending && <Icon icon={loaderCircleIcon} className="animate-spin" />}
                {!isStopPending && <Icon icon={squareIcon} />}
                {t(($) => $.records.workspace.stop)}
              </Button>
            )}
            {!isActive && !isEnded && (
              <Button variant="ghost" size="sm" asChild>
                <Link to="/records/$recordId/edit" params={{ recordId: summary.id }}>
                  <Icon icon={pencilIcon} />
                  {t(($) => $.records.workspace.edit)}
                </Link>
              </Button>
            )}
            {isEnded && (
              <Button
                variant="outline"
                size="sm"
                disabled={createRecord.isPending}
                onClick={() => void handleCopy()}
              >
                {createRecord.isPending && (
                  <Icon icon={loaderCircleIcon} className="animate-spin" />
                )}
                {!createRecord.isPending && <Icon icon={copyIcon} />}
                {t(($) => $.records.workspace.createCopy)}
              </Button>
            )}
          </div>
        </div>

        <div className="border-t border-border/70 px-4 py-3 md:px-5">
          <div className="flex flex-wrap items-center gap-2">
            <span className="mr-1 text-[10.5px] font-medium tracking-[0.04em] text-muted-foreground uppercase">
              {t(($) => $.records.editor.devices)}
            </span>
            <RecordDevicePicker
              devices={devices}
              selected={config.audio.record}
              active={summary.activeDevices}
              currentRecordId={summary.id}
              disabled={!isActive}
              onSelect={(device) => addDevice.mutate({ recordId: summary.id, device })}
              onRemove={(device) => removeDevice.mutate({ recordId: summary.id, device })}
            />
            {isActionPending && (
              <span className="inline-flex items-center gap-1 text-[10.5px] text-muted-foreground">
                <Icon icon={loaderCircleIcon} className="size-3 animate-spin" />
                {t(($) => $.records.editor.saving)}
              </span>
            )}
          </div>
        </div>

        <dl className="grid gap-px border-t bg-border/70 sm:grid-cols-2 min-[1060px]:grid-cols-4">
          <RecordMetric
            label={t(($) => $.records.workspace.language)}
            value={
              <span className="inline-flex items-center gap-1">
                <LanguageName code={config.recognition.sourceLanguage} />
                <span className="text-muted-foreground">→</span>
                <LanguageName code={config.translation.targetLanguage} />
              </span>
            }
          />
          <RecordMetric
            label={t(($) => $.records.workspace.recognition)}
            value={selectedAsr?.label ?? config.recognition.modelId}
          />
          <RecordMetric
            label={t(($) => $.records.workspace.translation)}
            value={translationLabel}
          />
          <RecordMetric label={t(($) => $.records.workspace.speakers)} value={speakerLabel} />
        </dl>
      </section>

      {summary.state === 'waitingForDevice' && (
        <Alert>
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.records.status.waitingForDevice)}</AlertTitle>
          <AlertDescription>{t(($) => $.records.workspace.waitingForDevice)}</AlertDescription>
        </Alert>
      )}

      {summary.state === 'failed' && summary.lastError && (
        <Alert variant="destructive">
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.records.status.failed)}</AlertTitle>
          <AlertDescription>{summary.lastError}</AlertDescription>
        </Alert>
      )}

      {summary.state === 'ended' && (
        <Alert>
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.records.status.ended)}</AlertTitle>
          <AlertDescription>{t(($) => $.records.workspace.ended)}</AlertDescription>
        </Alert>
      )}

      {shouldShowActionError && (
        <Alert variant="destructive">
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.workbench.errors.action)}</AlertTitle>
          <AlertDescription>{getErrorMessage(actionError)}</AlertDescription>
        </Alert>
      )}

      {createRecord.error && (
        <Alert variant="destructive">
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.records.editor.saveError)}</AlertTitle>
          <AlertDescription>{getErrorMessage(createRecord.error)}</AlertDescription>
        </Alert>
      )}
    </div>
  );
}
