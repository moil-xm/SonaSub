import { useTranslation } from '@sonasub/i18n';
import { useQuery } from '@tanstack/react-query';

import { assessTranslationReadiness } from '@/lib/model-readiness';
import { providersOptions } from '@/lib/providers';
import type { RecordDevice } from '@/lib/records';
import { audioDeviceKey } from '@/lib/session';
import type { AppConfig, Capabilities } from '@/lib/session';

import { RecordCheckRow } from './RecordCheckRow';

type RecordEditorChecksProps = {
  capabilities: Capabilities;
  config: AppConfig;
  devices: RecordDevice[];
};

export function RecordEditorChecks({ capabilities, config, devices }: RecordEditorChecksProps) {
  const { t } = useTranslation();
  const { data: providersView } = useQuery(providersOptions);
  const asr = capabilities.asrModels.find((model) => model.id === config.recognition.modelId);
  const translation = assessTranslationReadiness(
    capabilities,
    config,
    providersView?.providers ?? [],
  );
  const selectedDevices = config.audio.record.flatMap((selected) => {
    const device = devices.find(
      (candidate) => audioDeviceKey(candidate) === audioDeviceKey(selected),
    );
    return device ? [device] : [];
  });
  const selectedPlatformSupported = config.audio.record.every((selected) => {
    if (selected.kind === 'systemOutput') return capabilities.audio.systemCapture;
    return capabilities.audio.microphoneCapture;
  });
  const deviceReady =
    selectedPlatformSupported &&
    selectedDevices.length === config.audio.record.length &&
    selectedDevices.every((device) => device.captureSupported);
  const speakerReady =
    !config.speakers.enabled ||
    (capabilities.speakerEmbedding.available &&
      (!config.speakers.diarizationEnabled || capabilities.diarization.available));
  let deviceDetail: string = t(($) => $.records.editor.checksDevicesReady, {
    count: selectedDevices.length,
  });
  if (!selectedPlatformSupported) {
    deviceDetail = t(($) => $.records.editor.checksPlatformMissing);
  }
  if (selectedDevices.length === 0) {
    deviceDetail = t(($) => $.records.editor.checksDevicesMissing);
  } else if (!deviceReady) {
    deviceDetail = t(($) => $.records.editor.checksDevicesUnsupported);
  }
  let recognitionDetail: string = t(($) => $.records.editor.checksRecognitionReady);
  if (!asr?.available) {
    recognitionDetail = t(($) => $.records.editor.checksRecognitionMissing);
  }
  let translationDetail: string = t(($) => $.records.editor.checksTranslationReady);
  if (translation.issues.length > 0) {
    translationDetail = t(($) => $.records.editor.checksTranslationMissing);
  }
  let speakerDetail: string = t(($) => $.records.editor.checksSpeakerReady);
  if (!speakerReady) {
    speakerDetail = t(($) => $.records.editor.checksSpeakerMissing);
  }

  return (
    <div className="grid overflow-hidden rounded-lg border bg-card sm:grid-cols-2 xl:grid-cols-4">
      <RecordCheckRow
        label={t(($) => $.records.editor.checksDevices)}
        detail={deviceDetail}
        isReady={deviceReady}
      />
      <RecordCheckRow
        label={t(($) => $.records.editor.recognitionModel)}
        detail={recognitionDetail}
        isReady={Boolean(asr?.available)}
      />
      <RecordCheckRow
        label={t(($) => $.records.editor.translationModel)}
        detail={translationDetail}
        isReady={translation.issues.length === 0}
      />
      <RecordCheckRow
        label={t(($) => $.records.editor.identifySpeakers)}
        detail={speakerDetail}
        isReady={speakerReady}
      />
    </div>
  );
}
