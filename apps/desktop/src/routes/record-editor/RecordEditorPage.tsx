import { Icon } from '@iconify/react';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import { useTranslation } from '@sonasub/i18n';
import { Alert, AlertDescription, AlertTitle } from '@sonasub/ui/components/alert';
import { Button } from '@sonasub/ui/components/button';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';

import { getErrorMessage } from '@/lib/error';
import { isRecordActiveState, recordDevicesOptions, recordSnapshotOptions } from '@/lib/records';
import type { RecordDevice } from '@/lib/records';
import { sessionCapabilitiesOptions, sessionConfigOptions } from '@/lib/session';
import type { AppConfig, Capabilities } from '@/lib/session';
import { readRecordEditorDraft } from '@/stores/record-editor-draft';

import { RecordEditorForm } from './RecordEditorForm';

type RecordEditorPageProps = {
  recordId?: string;
};

function newRecordConfig(
  source: AppConfig,
  capabilities: Capabilities,
  devices: RecordDevice[],
): AppConfig {
  const config = structuredClone(source);
  config.recognition.mode = 'auto';
  config.speakers.enabled = false;

  const availableIds = new Set(
    devices.filter((device) => device.isAvailable).map((device) => device.id),
  );
  const preferredIds = config.audio.deviceIds.filter((id) => availableIds.has(id));
  if (preferredIds.length > 0) {
    config.audio.deviceIds = preferredIds;
  } else {
    const legacyDeviceId = config.audio.deviceId;
    const legacyAvailable = legacyDeviceId !== null && availableIds.has(legacyDeviceId);
    const defaultDevice = devices.find((device) => device.isDefault && device.isAvailable);
    if (legacyAvailable) {
      config.audio.deviceIds = [legacyDeviceId];
    } else if (defaultDevice) {
      config.audio.deviceIds = [defaultDevice.id];
    } else {
      config.audio.deviceIds = [];
    }
  }
  config.audio.deviceId = config.audio.deviceIds[0] ?? null;

  const asr = capabilities.asrModels.find((model) => model.id === config.recognition.modelId);
  if (!asr) {
    const fallback = capabilities.asrModels[0];
    if (fallback) {
      config.recognition.modelId = fallback.id;
      config.recognition.sourceLanguage = 'auto';
    }
  }

  const translation = capabilities.translationModels.find(
    (model) => model.id === config.translation.modelId,
  );
  if (!translation) {
    const fallback =
      capabilities.translationModels.find(
        (model) => model.targetLanguage === config.translation.targetLanguage,
      ) ?? capabilities.translationModels[0];
    if (fallback) {
      config.translation.modelId = fallback.id;
      config.translation.engine = fallback.engine;
      config.translation.targetLanguage = fallback.targetLanguage;
    }
  }
  return config;
}

export function RecordEditorPage({ recordId }: RecordEditorPageProps) {
  const { t } = useTranslation();
  let draftKey = 'new';
  if (recordId) draftKey = `edit:${recordId}`;
  const snapshotQuery = useQuery({
    ...recordSnapshotOptions(recordId ?? '__new__'),
    enabled: recordId !== undefined,
  });
  const configQuery = useQuery({
    ...sessionConfigOptions,
    enabled: recordId === undefined,
  });
  const capabilitiesQuery = useQuery(sessionCapabilitiesOptions);
  const devicesQuery = useQuery(recordDevicesOptions);

  const isInitialLoading =
    (!capabilitiesQuery.data && !capabilitiesQuery.error) ||
    (!devicesQuery.data && !devicesQuery.error) ||
    (recordId === undefined && !configQuery.data && !configQuery.error) ||
    (recordId !== undefined && !snapshotQuery.data && !snapshotQuery.error);

  if (isInitialLoading) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        {t(($) => $.records.editor.loading)}
      </div>
    );
  }

  const error =
    capabilitiesQuery.error ?? devicesQuery.error ?? configQuery.error ?? snapshotQuery.error;
  if (
    error ||
    !capabilitiesQuery.data ||
    !devicesQuery.data ||
    (recordId === undefined && !configQuery.data) ||
    (recordId !== undefined && !snapshotQuery.data)
  ) {
    return (
      <div className="h-full overflow-y-auto">
        <div className="mx-auto w-full max-w-[1040px] px-4 py-5 md:px-6">
          <Alert variant="destructive">
            <Icon icon={triangleAlertIcon} />
            <AlertTitle>{t(($) => $.records.editor.loadError)}</AlertTitle>
            <AlertDescription>{getErrorMessage(error)}</AlertDescription>
          </Alert>
          <Button variant="outline" size="sm" className="mt-4" asChild>
            <Link to="/">{t(($) => $.records.editor.cancel)}</Link>
          </Button>
        </div>
      </div>
    );
  }

  const snapshot = snapshotQuery.data;
  if (snapshot && isRecordActiveState(snapshot.summary.state)) {
    return (
      <div className="h-full overflow-y-auto">
        <div className="mx-auto w-full max-w-[1040px] px-4 py-5 md:px-6">
          <Alert>
            <Icon icon={triangleAlertIcon} />
            <AlertTitle>{snapshot.summary.name}</AlertTitle>
            <AlertDescription>{t(($) => $.records.editor.activeCannotEdit)}</AlertDescription>
          </Alert>
          <Button variant="outline" size="sm" className="mt-4" asChild>
            <Link to="/" search={{ record: snapshot.summary.id }}>
              {t(($) => $.records.editor.cancel)}
            </Link>
          </Button>
        </div>
      </div>
    );
  }

  if (snapshot && snapshot.summary.state === 'ended') {
    let message: string = t(($) => $.records.editor.endedCannotEdit);
    return (
      <div className="h-full overflow-y-auto">
        <div className="mx-auto w-full max-w-[1040px] px-4 py-5 md:px-6">
          <Alert>
            <Icon icon={triangleAlertIcon} />
            <AlertTitle>{snapshot.summary.name}</AlertTitle>
            <AlertDescription>{message}</AlertDescription>
          </Alert>
          <Button variant="outline" size="sm" className="mt-4" asChild>
            <Link to="/" search={{ record: snapshot.summary.id }}>
              {t(($) => $.records.editor.cancel)}
            </Link>
          </Button>
        </div>
      </div>
    );
  }

  const draft = readRecordEditorDraft(draftKey);
  let initialConfig: AppConfig;
  if (snapshot) {
    initialConfig = snapshot.config;
  } else {
    const configView = configQuery.data;
    if (!configView) {
      return (
        <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
          {t(($) => $.records.editor.loading)}
        </div>
      );
    }
    initialConfig = newRecordConfig(configView.config, capabilitiesQuery.data, devicesQuery.data);
  }
  if (draft) initialConfig = draft.config;
  const initialName = draft?.name ?? snapshot?.summary.name ?? '';

  return (
    <RecordEditorForm
      recordId={recordId}
      draftKey={draftKey}
      capabilities={capabilitiesQuery.data}
      devices={devicesQuery.data}
      initialConfig={initialConfig}
      initialName={initialName}
    />
  );
}
