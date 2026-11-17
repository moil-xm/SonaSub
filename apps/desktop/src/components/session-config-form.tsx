import { useEffect } from 'react';
import type { ReactNode } from 'react';
import { Icon } from '@iconify/react';
import checkIcon from '@iconify-icons/lucide/check';
import loaderCircleIcon from '@iconify-icons/lucide/loader-circle';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import { useForm, useSelector } from '@tanstack/react-form';
import { useTranslation } from '@sonasub/i18n';
import { Alert, AlertDescription, AlertTitle } from '@sonasub/ui/components/alert';
import { Button } from '@sonasub/ui/components/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@sonasub/ui/components/card';
import { Input } from '@sonasub/ui/components/input';
import { Label } from '@sonasub/ui/components/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@sonasub/ui/components/select';
import { Separator } from '@sonasub/ui/components/separator';
import { Switch } from '@sonasub/ui/components/switch';
import { useQuery } from '@tanstack/react-query';

import { getErrorMessage } from '@/lib/error';
import { assessModelReadiness } from '@/lib/model-readiness';
import {
  sessionCapabilitiesOptions,
  sessionConfigOptions,
  sessionSnapshotOptions,
  useSaveConfig,
} from '@/lib/session';
import type {
  AppConfig,
  AsrMode,
  Capabilities,
  ConfigView,
  TranslationMode,
  TranslationTrigger,
} from '@/lib/session.type';

import { ReadinessIssues } from './readiness-issues';

type SessionConfigFormProps = {
  onDirtyChange: (isDirty: boolean) => void;
};

type SettingFieldProps = {
  id: string;
  label: string;
  hint?: string;
  children: ReactNode;
};

type SettingSelectProps = {
  id: string;
  value: string;
  disabled: boolean;
  onChange: (value: string) => void;
  children: ReactNode;
};

type LanguageNameProps = {
  code: string;
};

function LanguageName({ code }: LanguageNameProps) {
  const { t } = useTranslation();
  if (code === 'auto') return t(($) => $.languages.auto);
  if (code === 'zh' || code === 'zh-Hans') return t(($) => $.languages.chinese);
  if (code === 'en') return t(($) => $.languages.english);
  if (code === 'ja') return t(($) => $.languages.japanese);
  if (code === 'ko') return t(($) => $.languages.korean);
  if (code === 'yue') return t(($) => $.languages.cantonese);
  return code;
}

function SettingField({ id, label, hint, children }: SettingFieldProps) {
  return (
    <div className="space-y-2">
      <Label htmlFor={id}>{label}</Label>
      {children}
      {hint && <p className="text-xs leading-5 text-muted-foreground">{hint}</p>}
    </div>
  );
}

function SettingSelect({ id, value, disabled, onChange, children }: SettingSelectProps) {
  return (
    <Select value={value} onValueChange={onChange} disabled={disabled}>
      <SelectTrigger id={id} className="w-full">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>{children}</SelectContent>
    </Select>
  );
}

function asAsrMode(value: string): AsrMode {
  if (value === 'streaming') return 'streaming';
  if (value === 'segmented') return 'segmented';
  return 'auto';
}

function asTranslationMode(value: string): TranslationMode {
  if (value === 'streaming') return 'streaming';
  if (value === 'text') return 'text';
  return 'auto';
}

function asTranslationTrigger(value: string): TranslationTrigger {
  if (value === 'stableSegment') return 'stableSegment';
  return 'finalOnly';
}

function parseNumber(value: string): number {
  return Number(value);
}

export function SessionConfigForm({ onDirtyChange }: SessionConfigFormProps) {
  const { t } = useTranslation();
  const { data: capabilities, error: capabilitiesError } = useQuery(sessionCapabilitiesOptions);
  const { data: configView, error: configError } = useQuery(sessionConfigOptions);

  if (!capabilities || !configView) {
    return (
      <Card>
        <CardContent className="flex min-h-72 items-center justify-center text-sm text-muted-foreground">
          {t(($) => $.config.loading)}
        </CardContent>
      </Card>
    );
  }

  return (
    <ConfigForm
      key={configView.revision}
      capabilities={capabilities}
      configView={configView}
      configError={configError}
      capabilitiesError={capabilitiesError}
      onDirtyChange={onDirtyChange}
    />
  );
}

type ConfigFormProps = {
  capabilities: Capabilities;
  configView: ConfigView;
  configError: Error | null;
  capabilitiesError: Error | null;
  onDirtyChange: (isDirty: boolean) => void;
};

function ConfigForm({
  capabilities,
  configView,
  configError,
  capabilitiesError,
  onDirtyChange,
}: ConfigFormProps) {
  const { t } = useTranslation();
  const { data: snapshot } = useQuery(sessionSnapshotOptions);
  const saveConfig = useSaveConfig();
  const form = useForm({
    defaultValues: configView.config,
    onSubmit: async ({ value }) => {
      const saved = await saveConfig.mutateAsync(value);
      form.reset(saved.config);
    },
  });
  const isDirty = useSelector(form.store, (state) => state.isDirty);
  const isSubmitting = useSelector(form.store, (state) => state.isSubmitting);
  const speakers = useSelector(form.store, (state) => state.values.speakers);
  const formConfig: AppConfig = useSelector(form.store, (state) => state.values);
  const readiness = assessModelReadiness(capabilities, formConfig);
  const selectedModel = readiness.selectedAsr;
  let isLocked = false;
  const status = snapshot?.status;
  if (status) {
    isLocked =
      status.state === 'starting' || status.state === 'running' || status.state === 'stopping';
  }
  const isDisabled = isLocked || isSubmitting || saveConfig.isPending;
  const missingSelectedModel = selectedModel?.missingFiles ?? [];
  const missingTranslation = readiness.selectedTranslationModels.flatMap(
    (model) => model.missingFiles,
  );

  useEffect(() => {
    onDirtyChange(isDirty);
  }, [isDirty, onDirtyChange]);

  useEffect(() => {
    return () => onDirtyChange(false);
  }, [onDirtyChange]);

  let saveLabel: string = t(($) => $.config.save);
  if (saveConfig.isPending) saveLabel = t(($) => $.config.saving);

  return (
    <Card className="overflow-hidden">
      <CardHeader className="border-b">
        <div className="flex items-start justify-between gap-3">
          <div className="space-y-1">
            <CardTitle className="text-base">{t(($) => $.config.title)}</CardTitle>
            <CardDescription>{t(($) => $.config.description)}</CardDescription>
          </div>
          {isDirty && (
            <span className="shrink-0 rounded-full bg-warning/15 px-2 py-1 text-[11px] font-medium text-warning">
              {t(($) => $.config.unsaved)}
            </span>
          )}
        </div>
        {isLocked && (
          <p className="rounded-md bg-muted px-3 py-2 text-xs leading-5 text-muted-foreground">
            {t(($) => $.config.locked)}
          </p>
        )}
      </CardHeader>

      <CardContent className="space-y-6 p-5">
        {configView.loadError && (
          <Alert variant="destructive">
            <Icon icon={triangleAlertIcon} />
            <AlertTitle>{t(($) => $.config.loadError)}</AlertTitle>
            <AlertDescription>{configView.loadError}</AlertDescription>
          </Alert>
        )}

        {capabilitiesError && (
          <Alert variant="destructive">
            <Icon icon={triangleAlertIcon} />
            <AlertTitle>{t(($) => $.workbench.errors.capabilities)}</AlertTitle>
            <AlertDescription>{getErrorMessage(capabilitiesError)}</AlertDescription>
          </Alert>
        )}

        {configError && (
          <Alert variant="destructive">
            <Icon icon={triangleAlertIcon} />
            <AlertTitle>{t(($) => $.workbench.errors.config)}</AlertTitle>
            <AlertDescription>{getErrorMessage(configError)}</AlertDescription>
          </Alert>
        )}

        {!readiness.canStart && (
          <Alert>
            <Icon icon={triangleAlertIcon} />
            <AlertTitle>{t(($) => $.config.readiness)}</AlertTitle>
            <AlertDescription>
              <ReadinessIssues issues={readiness.issues} />
            </AlertDescription>
          </Alert>
        )}

        <form
          className="space-y-7"
          onSubmit={(event) => {
            event.preventDefault();
            event.stopPropagation();
            void form.handleSubmit();
          }}
        >
          <section className="space-y-4">
            <div>
              <h3 className="text-sm font-semibold">{t(($) => $.config.audioSection)}</h3>
              <p className="mt-1 text-xs leading-5 text-muted-foreground">
                {t(($) => $.config.audioHint, {
                  platform: capabilities.audio.supportedPlatform,
                })}
              </p>
            </div>
            <form.Field name="audio.deviceId">
              {(field) => {
                const selectedDevice = field.state.value ?? '__system_default__';
                return (
                  <SettingField
                    id="audio-device"
                    label={t(($) => $.config.audioOutput)}
                    hint={t(($) => $.config.audioBackend, {
                      backend: capabilities.audio.backend,
                    })}
                  >
                    <SettingSelect
                      id="audio-device"
                      value={selectedDevice}
                      disabled={isDisabled}
                      onChange={(value) => {
                        if (value === '__system_default__') field.handleChange(null);
                        else field.handleChange(value);
                      }}
                    >
                      <SelectItem value="__system_default__">
                        {t(($) => $.config.systemDefault)}
                      </SelectItem>
                      {capabilities.audio.devices.map((device) => (
                        <SelectItem key={device.id} value={device.id}>
                          {device.name}
                        </SelectItem>
                      ))}
                    </SettingSelect>
                  </SettingField>
                );
              }}
            </form.Field>
          </section>

          <Separator />

          <section className="space-y-4">
            <div>
              <h3 className="text-sm font-semibold">{t(($) => $.config.recognitionSection)}</h3>
              <p className="mt-1 text-xs leading-5 text-muted-foreground">
                {t(($) => $.config.recognitionHint)}
              </p>
            </div>

            <form.Field name="recognition.modelId">
              {(field) => (
                <SettingField
                  id="asr-model"
                  label={t(($) => $.config.asrModel)}
                  hint={t(($) => $.config.modelAvailability, {
                    version: selectedModel?.version ?? '',
                  })}
                >
                  <SettingSelect
                    id="asr-model"
                    value={field.state.value}
                    disabled={isDisabled}
                    onChange={(value) => {
                      field.handleChange(value);
                      const nextModel = capabilities.asrModels.find((model) => model.id === value);
                      if (!nextModel) return;
                      const currentMode = form.state.values.recognition.mode;
                      if (currentMode !== 'auto' && currentMode !== nextModel.kind) {
                        form.setFieldValue('recognition.mode', 'auto');
                      }
                      const currentLanguage = form.state.values.recognition.sourceLanguage;
                      if (
                        currentLanguage !== 'auto' &&
                        !nextModel.sourceLanguages.includes(currentLanguage)
                      ) {
                        form.setFieldValue('recognition.sourceLanguage', 'auto');
                      }
                    }}
                  >
                    {capabilities.asrModels.map((model) => (
                      <SelectItem key={model.id} value={model.id}>
                        {model.label}
                        {!model.available && ` · ${t(($) => $.config.missing)}`}
                      </SelectItem>
                    ))}
                  </SettingSelect>
                </SettingField>
              )}
            </form.Field>

            {missingSelectedModel.length > 0 && (
              <p className="text-xs leading-5 text-warning">
                {t(($) => $.config.modelMissing, { files: missingSelectedModel.join(', ') })}
              </p>
            )}

            <form.Field name="recognition.mode">
              {(field) => {
                let modeHint: string = t(($) => $.config.asrModeAutoHint);
                if (field.state.value !== 'auto') {
                  modeHint = t(($) => $.config.asrModeExplicitHint);
                }
                return (
                  <SettingField id="asr-mode" label={t(($) => $.config.asrMode)} hint={modeHint}>
                    <SettingSelect
                      id="asr-mode"
                      value={field.state.value}
                      disabled={isDisabled}
                      onChange={(value) => field.handleChange(asAsrMode(value))}
                    >
                      <SelectItem value="auto">{t(($) => $.config.asrModes.auto)}</SelectItem>
                      <SelectItem value="streaming" disabled={selectedModel?.kind !== 'streaming'}>
                        {t(($) => $.config.asrModes.streaming)}
                      </SelectItem>
                      <SelectItem value="segmented" disabled={selectedModel?.kind !== 'segmented'}>
                        {t(($) => $.config.asrModes.segmented)}
                      </SelectItem>
                    </SettingSelect>
                  </SettingField>
                );
              }}
            </form.Field>

            <form.Field name="recognition.sourceLanguage">
              {(field) => (
                <SettingField
                  id="source-language"
                  label={t(($) => $.config.sourceLanguage)}
                  hint={t(($) => $.config.sourceLanguageHint)}
                >
                  <SettingSelect
                    id="source-language"
                    value={field.state.value}
                    disabled={isDisabled}
                    onChange={field.handleChange}
                  >
                    <SelectItem value="auto">{t(($) => $.languages.auto)}</SelectItem>
                    {selectedModel?.sourceLanguages.map((language) => (
                      <SelectItem key={language} value={language}>
                        <LanguageName code={language} />
                      </SelectItem>
                    ))}
                  </SettingSelect>
                </SettingField>
              )}
            </form.Field>
          </section>

          <Separator />

          <section className="space-y-4">
            <div>
              <h3 className="text-sm font-semibold">{t(($) => $.config.translationSection)}</h3>
              <p className="mt-1 text-xs leading-5 text-muted-foreground">
                {t(($) => $.config.translationHint, { engine: capabilities.translationEngine })}
              </p>
            </div>

            <form.Field name="translation.targetLanguage">
              {(field) => (
                <SettingField
                  id="target-language"
                  label={t(($) => $.config.targetLanguage)}
                  hint={t(($) => $.config.targetLanguageHint)}
                >
                  <SettingSelect
                    id="target-language"
                    value={field.state.value}
                    disabled={isDisabled}
                    onChange={field.handleChange}
                  >
                    {capabilities.translationTargets.map((language) => (
                      <SelectItem key={language.code} value={language.code}>
                        <LanguageName code={language.code} />
                      </SelectItem>
                    ))}
                  </SettingSelect>
                </SettingField>
              )}
            </form.Field>

            {missingTranslation.length > 0 && (
              <p className="text-xs leading-5 text-warning">
                {t(($) => $.config.modelMissing, { files: missingTranslation.join(', ') })}
              </p>
            )}

            <form.Field name="translation.mode">
              {(field) => {
                const selectedMode = capabilities.translationModes.find(
                  (mode) => mode.mode === field.state.value,
                );
                let modeHint: string =
                  selectedMode?.reason ?? t(($) => $.config.translationModeHint);
                if (field.state.value === 'auto') {
                  modeHint = t(($) => $.config.translationAutoHint);
                }
                return (
                  <SettingField
                    id="translation-mode"
                    label={t(($) => $.config.translationMode)}
                    hint={modeHint}
                  >
                    <SettingSelect
                      id="translation-mode"
                      value={field.state.value}
                      disabled={isDisabled}
                      onChange={(value) => field.handleChange(asTranslationMode(value))}
                    >
                      {capabilities.translationModes.map((mode) => (
                        <SelectItem key={mode.mode} value={mode.mode} disabled={!mode.available}>
                          {mode.mode === 'auto' && t(($) => $.config.translationModes.auto)}
                          {mode.mode === 'text' && t(($) => $.config.translationModes.text)}
                          {mode.mode === 'streaming' &&
                            t(($) => $.config.translationModes.streaming)}
                        </SelectItem>
                      ))}
                    </SettingSelect>
                  </SettingField>
                );
              }}
            </form.Field>

            <form.Field name="translation.trigger">
              {(field) => {
                let triggerHint: string = t(($) => $.config.triggerFinalHint);
                if (field.state.value === 'stableSegment') {
                  triggerHint = t(($) => $.config.triggerStableHint);
                }
                return (
                  <SettingField
                    id="translation-trigger"
                    label={t(($) => $.config.trigger)}
                    hint={triggerHint}
                  >
                    <SettingSelect
                      id="translation-trigger"
                      value={field.state.value}
                      disabled={isDisabled}
                      onChange={(value) => field.handleChange(asTranslationTrigger(value))}
                    >
                      <SelectItem value="finalOnly">
                        {t(($) => $.config.triggers.finalOnly)}
                      </SelectItem>
                      <SelectItem value="stableSegment">
                        {t(($) => $.config.triggers.stableSegment)}
                      </SelectItem>
                    </SettingSelect>
                  </SettingField>
                );
              }}
            </form.Field>
          </section>

          <Separator />

          <section className="space-y-4">
            <div>
              <h3 className="text-sm font-semibold">{t(($) => $.config.speakerSection)}</h3>
              <p className="mt-1 text-xs leading-5 text-muted-foreground">
                {t(($) => $.config.speakerHint)}
              </p>
            </div>

            <form.Field name="speakers.enabled">
              {(field) => (
                <div className="flex items-center justify-between gap-4 rounded-lg border p-3">
                  <div className="space-y-1">
                    <Label htmlFor="speakers-enabled">{t(($) => $.config.speakersEnabled)}</Label>
                    <p className="text-xs text-muted-foreground">
                      {t(($) => $.config.speakersEnabledHint)}
                    </p>
                  </div>
                  <Switch
                    id="speakers-enabled"
                    checked={field.state.value}
                    disabled={isDisabled}
                    onCheckedChange={field.handleChange}
                  />
                </div>
              )}
            </form.Field>

            <form.Field name="speakers.diarizationEnabled">
              {(field) => (
                <div className="flex items-center justify-between gap-4 rounded-lg border p-3">
                  <div className="space-y-1">
                    <Label htmlFor="diarization-enabled">{t(($) => $.config.diarization)}</Label>
                    <p className="text-xs text-muted-foreground">
                      {t(($) => $.config.diarizationHint)}
                    </p>
                  </div>
                  <Switch
                    id="diarization-enabled"
                    checked={field.state.value}
                    disabled={isDisabled || !speakers.enabled}
                    onCheckedChange={field.handleChange}
                  />
                </div>
              )}
            </form.Field>
          </section>

          <details className="rounded-lg border">
            <summary className="cursor-pointer list-none px-4 py-3 text-sm font-medium">
              {t(($) => $.config.advancedSection)}
            </summary>
            <div className="grid gap-4 border-t p-4 sm:grid-cols-2">
              <form.Field name="recognition.maxSegmentSecs">
                {(field) => (
                  <SettingField
                    id="max-segment"
                    label={t(($) => $.config.maxSegment)}
                    hint={t(($) => $.config.maxSegmentHint)}
                  >
                    <Input
                      id="max-segment"
                      type="number"
                      min={1}
                      max={30}
                      step={0.5}
                      value={field.state.value}
                      disabled={isDisabled}
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingField>
                )}
              </form.Field>

              <form.Field name="recognition.stableWaitMs">
                {(field) => (
                  <SettingField
                    id="stable-wait"
                    label={t(($) => $.config.stableWait)}
                    hint={t(($) => $.config.stableWaitHint)}
                  >
                    <Input
                      id="stable-wait"
                      type="number"
                      min={200}
                      max={10_000}
                      step={100}
                      value={field.state.value}
                      disabled={isDisabled}
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingField>
                )}
              </form.Field>

              <form.Field name="recognition.vadThreshold">
                {(field) => (
                  <SettingField id="vad-threshold" label={t(($) => $.config.vadThreshold)}>
                    <Input
                      id="vad-threshold"
                      type="number"
                      min={0.05}
                      max={0.95}
                      step={0.05}
                      value={field.state.value}
                      disabled={isDisabled || readiness.selectedMode !== 'segmented'}
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingField>
                )}
              </form.Field>

              <form.Field name="recognition.minSilenceSecs">
                {(field) => (
                  <SettingField id="min-silence" label={t(($) => $.config.minSilence)}>
                    <Input
                      id="min-silence"
                      type="number"
                      min={0.1}
                      max={2}
                      step={0.05}
                      value={field.state.value}
                      disabled={isDisabled || readiness.selectedMode !== 'segmented'}
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingField>
                )}
              </form.Field>

              <form.Field name="recognition.minSpeechSecs">
                {(field) => (
                  <SettingField id="min-speech" label={t(($) => $.config.minSpeech)}>
                    <Input
                      id="min-speech"
                      type="number"
                      min={0.1}
                      max={2}
                      step={0.05}
                      value={field.state.value}
                      disabled={isDisabled || readiness.selectedMode !== 'segmented'}
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingField>
                )}
              </form.Field>

              <form.Field name="speakers.windowSecs">
                {(field) => (
                  <SettingField
                    id="diarization-window"
                    label={t(($) => $.config.window)}
                    hint={t(($) => $.config.windowHint)}
                  >
                    <Input
                      id="diarization-window"
                      type="number"
                      min={5}
                      max={60}
                      step={1}
                      value={field.state.value}
                      disabled={isDisabled || !speakers.enabled || !speakers.diarizationEnabled}
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingField>
                )}
              </form.Field>

              <form.Field name="speakers.maxSpeakers">
                {(field) => (
                  <SettingField id="max-speakers" label={t(($) => $.config.maxSpeakers)}>
                    <Input
                      id="max-speakers"
                      type="number"
                      min={1}
                      max={16}
                      step={1}
                      value={field.state.value}
                      disabled={isDisabled || !speakers.enabled}
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingField>
                )}
              </form.Field>

              <form.Field name="speakers.similarityThreshold">
                {(field) => (
                  <SettingField
                    id="speaker-threshold"
                    label={t(($) => $.config.similarity)}
                    hint={t(($) => $.config.similarityHint)}
                  >
                    <Input
                      id="speaker-threshold"
                      type="number"
                      min={0.3}
                      max={0.9}
                      step={0.05}
                      value={field.state.value}
                      disabled={isDisabled || !speakers.enabled}
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingField>
                )}
              </form.Field>

              <form.Field name="ui.historyLimit">
                {(field) => (
                  <SettingField
                    id="history-limit"
                    label={t(($) => $.config.historyLimit)}
                    hint={t(($) => $.config.historyLimitHint)}
                  >
                    <Input
                      id="history-limit"
                      type="number"
                      min={50}
                      max={2_000}
                      step={50}
                      value={field.state.value}
                      disabled={isDisabled}
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingField>
                )}
              </form.Field>
            </div>
          </details>

          <div className="space-y-3">
            <p className="break-all text-xs leading-5 text-muted-foreground">
              {t(($) => $.config.modelsDirectory, { path: capabilities.modelsDir })}
            </p>
            {saveConfig.error && (
              <Alert variant="destructive">
                <Icon icon={triangleAlertIcon} />
                <AlertTitle>{t(($) => $.config.saveError)}</AlertTitle>
                <AlertDescription>{getErrorMessage(saveConfig.error)}</AlertDescription>
              </Alert>
            )}
            {saveConfig.isSuccess && !isDirty && (
              <p className="flex items-center gap-1.5 text-xs text-success">
                <Icon icon={checkIcon} />
                {t(($) => $.config.saved)}
              </p>
            )}
            <Button type="submit" className="w-full" disabled={isDisabled || !isDirty}>
              {saveConfig.isPending && <Icon icon={loaderCircleIcon} className="animate-spin" />}
              {saveLabel}
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  );
}
