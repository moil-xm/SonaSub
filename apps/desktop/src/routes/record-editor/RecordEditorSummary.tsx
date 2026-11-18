import { useTranslation } from '@sonasub/i18n';

import { LanguageName } from '@/components/session-config-form';
import type { AppConfig, Capabilities } from '@/lib/session';
import type { RecordDevice } from '@/lib/records';

import { SummaryRow } from './SummaryRow';

type RecordEditorSummaryProps = {
  capabilities: Capabilities;
  config: AppConfig;
  devices: RecordDevice[];
  name: string;
};

export function RecordEditorSummary({
  capabilities,
  config,
  devices,
  name,
}: RecordEditorSummaryProps) {
  const { t } = useTranslation();
  const asr = capabilities.asrModels.find((model) => model.id === config.recognition.modelId);
  const translation = capabilities.translationModels.find(
    (model) => model.id === config.translation.modelId,
  );
  const firstDevice = devices.find((device) => device.id === config.audio.deviceIds[0]);
  let displayName = name.trim();
  if (!displayName) displayName = firstDevice?.name ?? t(($) => $.records.editor.namePlaceholder);
  const sourceLanguage = config.recognition.sourceLanguage;
  const deviceNames = config.audio.deviceIds.map((id) => {
    const device = devices.find((candidate) => candidate.id === id);
    return device?.name ?? id;
  });
  let deviceSummary: string = t(($) => $.records.editor.deviceCount, {
    count: config.audio.deviceIds.length,
  });
  if (deviceNames.length > 0) deviceSummary = deviceNames.join(' · ');
  let speakerSummary: string = t(($) => $.records.editor.summaryOff);
  if (config.speakers.enabled) speakerSummary = t(($) => $.records.editor.summaryOn);

  return (
    <aside className="hidden w-[264px] shrink-0 xl:block">
      <div className="sticky top-4 overflow-hidden rounded-xl border bg-card">
        <div className="border-b px-4 py-3">
          <p className="text-[11px] font-medium tracking-[0.04em] text-muted-foreground uppercase">
            {t(($) => $.records.editor.summaryTitle)}
          </p>
          <p className="mt-1.5 truncate text-[14px] font-semibold">{displayName}</p>
          <p className="mt-1 text-[10.5px] text-muted-foreground">
            {t(($) => $.records.editor.summaryNotSaved)}
          </p>
        </div>
        <dl className="divide-y divide-border/70 text-[11px]">
          <SummaryRow label={t(($) => $.records.editor.summaryDevices)}>
            <span className="line-clamp-2 text-right">{deviceSummary}</span>
          </SummaryRow>
          <SummaryRow label={t(($) => $.records.editor.summaryLanguage)}>
            <span className="text-right">
              <LanguageName code={sourceLanguage} />
              {' → '}
              <LanguageName code={config.translation.targetLanguage} />
            </span>
          </SummaryRow>
          <SummaryRow label={t(($) => $.records.editor.summaryRecognition)}>
            <span className="text-right">{asr?.label ?? config.recognition.modelId}</span>
          </SummaryRow>
          <SummaryRow label={t(($) => $.records.editor.summaryTranslation)}>
            <span className="text-right">{translation?.label ?? config.translation.modelId}</span>
          </SummaryRow>
          <SummaryRow label={t(($) => $.records.editor.summarySpeakers)}>
            <span className="text-right">{speakerSummary}</span>
          </SummaryRow>
        </dl>
      </div>
    </aside>
  );
}
