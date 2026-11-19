import { useCallback } from 'react';
import { Icon } from '@iconify/react';
import checkIcon from '@iconify-icons/lucide/check';
import chevronRightIcon from '@iconify-icons/lucide/chevron-right';
import loaderCircleIcon from '@iconify-icons/lucide/loader-circle';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import { useForm, useStore } from '@tanstack/react-form';
import { useTranslation } from '@sonasub/i18n';
import { Alert, AlertDescription, AlertTitle } from '@sonasub/ui/components/alert';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@sonasub/ui/components/alert-dialog';
import { Button } from '@sonasub/ui/components/button';
import { Input } from '@sonasub/ui/components/input';
import { SelectItem } from '@sonasub/ui/components/select';
import { Switch } from '@sonasub/ui/components/switch';
import { useQuery } from '@tanstack/react-query';
import { useBlocker } from '@tanstack/react-router';

import { ReadinessIssues } from '@/components/readiness-issues';
import { SettingsList, SettingsRow, SettingsSection } from '@/components/settings-section';
import { getErrorMessage } from '@/lib/error';
import { assessModelReadiness } from '@/lib/model-readiness';
import { sessionSnapshotOptions, useSaveConfig } from '@/lib/session';
import type {
  AppConfig,
  AsrMode,
  Capabilities,
  ConfigView,
  TranslationMode,
  TranslationTrigger,
} from '@/lib/session';

import { LanguageName } from './LanguageName';
import { SettingSelect } from './SettingSelect';

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

type ConfigFormProps = {
  capabilities: Capabilities;
  configView: ConfigView;
  configError: Error | null;
  capabilitiesError: Error | null;
};

export function ConfigForm({
  capabilities,
  configView,
  configError,
  capabilitiesError,
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
  const isDirty = useStore(form.store, (state) => state.isDirty);
  const isSubmitting = useStore(form.store, (state) => state.isSubmitting);
  const speakers = useStore(form.store, (state) => state.values.speakers);
  const formConfig: AppConfig = useStore(form.store, (state) => state.values);
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
  const shouldBlockNavigation = useCallback(() => isDirty, [isDirty]);
  const blocker = useBlocker({
    shouldBlockFn: shouldBlockNavigation,
    withResolver: true,
    enableBeforeUnload: false,
  });

  let saveLabel: string = t(($) => $.config.save);
  if (saveConfig.isPending) saveLabel = t(($) => $.config.saving);

  let modelDescription: string = t(($) => $.config.modelAvailability, {
    version: selectedModel?.version ?? '',
  });
  if (missingSelectedModel.length > 0) {
    modelDescription = `${modelDescription} ${t(($) => $.config.modelMissing, {
      files: missingSelectedModel.join(', '),
    })}`;
  }
  let targetLanguageDescription: string = t(($) => $.config.targetLanguageHint);
  if (missingTranslation.length > 0) {
    targetLanguageDescription = `${targetLanguageDescription} ${t(($) => $.config.modelMissing, {
      files: missingTranslation.join(', '),
    })}`;
  }

  return (
    <div className="h-full overflow-y-auto">
      <form
        className="mx-auto flex min-h-full w-full max-w-[1040px] flex-col px-4 pb-5 md:px-6"
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void form.handleSubmit();
        }}
      >
        <div className="flex flex-col gap-3 py-4 md:flex-row md:items-start md:justify-between">
          <p className="max-w-2xl text-xs leading-5 text-muted-foreground">
            {t(($) => $.config.description)}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            {isLocked && (
              <span className="rounded-full bg-muted px-2 py-1 text-[11px] text-muted-foreground">
                {t(($) => $.config.locked)}
              </span>
            )}
            {isDirty && (
              <span className="rounded-full bg-warning/15 px-2 py-1 text-[11px] font-medium text-warning">
                {t(($) => $.config.unsaved)}
              </span>
            )}
          </div>
        </div>

        <div className="space-y-6">
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

          {saveConfig.error && (
            <Alert variant="destructive">
              <Icon icon={triangleAlertIcon} />
              <AlertTitle>{t(($) => $.config.saveError)}</AlertTitle>
              <AlertDescription>{getErrorMessage(saveConfig.error)}</AlertDescription>
            </Alert>
          )}

          <SettingsSection title={t(($) => $.config.audioSection)}>
            <SettingsList>
              <form.Field name="audio.deviceId">
                {(field) => {
                  const selectedDevice = field.state.value ?? '__system_default__';
                  return (
                    <SettingsRow
                      labelFor="audio-device"
                      title={t(($) => $.config.audioOutput)}
                      description={`${t(($) => $.config.audioBackend, {
                        backend: capabilities.audio.backend,
                      })} ${t(($) => $.config.audioHint, {
                        platform: capabilities.audio.supportedPlatform,
                      })}`}
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
                    </SettingsRow>
                  );
                }}
              </form.Field>
            </SettingsList>
          </SettingsSection>

          <SettingsSection title={t(($) => $.config.recognitionSection)}>
            <SettingsList>
              <form.Field name="recognition.modelId">
                {(field) => (
                  <SettingsRow
                    labelFor="asr-model"
                    title={t(($) => $.config.asrModel)}
                    description={modelDescription}
                  >
                    <SettingSelect
                      id="asr-model"
                      value={field.state.value}
                      disabled={isDisabled}
                      onChange={(value) => {
                        field.handleChange(value);
                        const nextModel = capabilities.asrModels.find(
                          (model) => model.id === value,
                        );
                        if (!nextModel) return;
                        const currentMode = form.state.values.recognition.mode;
                        if (currentMode !== 'auto' && currentMode !== nextModel.kind) {
                          form.setFieldValue('recognition.mode', 'auto');
                        }
                        const currentLanguage = form.state.values.recognition.sourceLanguage;
                        if (
                          currentLanguage !== 'auto' &&
                          (!nextModel.sourceLanguages.includes(currentLanguage) ||
                            !nextModel.supportsSourceLanguage)
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
                  </SettingsRow>
                )}
              </form.Field>

              <form.Field name="recognition.mode">
                {(field) => {
                  let modeHint: string = t(($) => $.config.asrModeAutoHint);
                  if (field.state.value !== 'auto') {
                    modeHint = t(($) => $.config.asrModeExplicitHint);
                  }
                  if (selectedModel?.kind === 'segmented') {
                    modeHint = `${modeHint} ${t(($) => $.config.segmentedNoPartialHint)}`;
                  }
                  return (
                    <SettingsRow
                      labelFor="asr-mode"
                      title={t(($) => $.config.asrMode)}
                      description={modeHint}
                    >
                      <SettingSelect
                        id="asr-mode"
                        value={field.state.value}
                        disabled={isDisabled}
                        onChange={(value) => field.handleChange(asAsrMode(value))}
                      >
                        <SelectItem value="auto">{t(($) => $.config.asrModes.auto)}</SelectItem>
                        <SelectItem
                          value="streaming"
                          disabled={selectedModel?.kind !== 'streaming'}
                        >
                          {t(($) => $.config.asrModes.streaming)}
                        </SelectItem>
                        <SelectItem
                          value="segmented"
                          disabled={selectedModel?.kind !== 'segmented'}
                        >
                          {t(($) => $.config.asrModes.segmented)}
                        </SelectItem>
                      </SettingSelect>
                    </SettingsRow>
                  );
                }}
              </form.Field>

              <form.Field name="recognition.sourceLanguage">
                {(field) => {
                  // A model that detects the language per utterance cannot
                  // honour a forced source language, so the control is
                  // locked on automatic and explains why.
                  const supportsSourceLanguage = selectedModel?.supportsSourceLanguage ?? false;
                  const description = supportsSourceLanguage
                    ? t(($) => $.config.sourceLanguageHint)
                    : t(($) => $.config.sourceLanguageAutoOnly);
                  return (
                    <SettingsRow
                      labelFor="source-language"
                      title={t(($) => $.config.sourceLanguage)}
                      description={description}
                    >
                      <SettingSelect
                        id="source-language"
                        value={supportsSourceLanguage ? field.state.value : 'auto'}
                        disabled={isDisabled || !supportsSourceLanguage}
                        onChange={field.handleChange}
                      >
                        <SelectItem value="auto">{t(($) => $.languages.auto)}</SelectItem>
                        {supportsSourceLanguage &&
                          selectedModel?.sourceLanguages.map((language) => (
                            <SelectItem key={language} value={language}>
                              <LanguageName code={language} />
                            </SelectItem>
                          ))}
                      </SettingSelect>
                    </SettingsRow>
                  );
                }}
              </form.Field>
            </SettingsList>
          </SettingsSection>

          <SettingsSection title={t(($) => $.config.translationSection)}>
            <SettingsList>
              <form.Field name="translation.targetLanguage">
                {(field) => (
                  <SettingsRow
                    labelFor="target-language"
                    title={t(($) => $.config.targetLanguage)}
                    description={targetLanguageDescription}
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
                  </SettingsRow>
                )}
              </form.Field>

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
                    <SettingsRow
                      labelFor="translation-mode"
                      title={t(($) => $.config.translationMode)}
                      description={modeHint}
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
                    </SettingsRow>
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
                    <SettingsRow
                      labelFor="translation-trigger"
                      title={t(($) => $.config.trigger)}
                      description={triggerHint}
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
                    </SettingsRow>
                  );
                }}
              </form.Field>
            </SettingsList>
          </SettingsSection>

          <SettingsSection title={t(($) => $.config.speakerSection)}>
            <SettingsList>
              <form.Field name="speakers.enabled">
                {(field) => (
                  <SettingsRow
                    labelFor="speakers-enabled"
                    title={t(($) => $.config.speakersEnabled)}
                    description={t(($) => $.config.speakersEnabledHint)}
                  >
                    <Switch
                      id="speakers-enabled"
                      checked={field.state.value}
                      disabled={isDisabled}
                      onCheckedChange={field.handleChange}
                    />
                  </SettingsRow>
                )}
              </form.Field>

              <form.Field name="speakers.diarizationEnabled">
                {(field) => (
                  <SettingsRow
                    labelFor="diarization-enabled"
                    title={t(($) => $.config.diarization)}
                    description={t(($) => $.config.diarizationHint)}
                  >
                    <Switch
                      id="diarization-enabled"
                      checked={field.state.value}
                      disabled={isDisabled || !speakers.enabled}
                      onCheckedChange={field.handleChange}
                    />
                  </SettingsRow>
                )}
              </form.Field>

              <form.Field name="speakers.maxSpeakers">
                {(field) => (
                  <SettingsRow labelFor="max-speakers" title={t(($) => $.config.maxSpeakers)}>
                    <Input
                      id="max-speakers"
                      type="number"
                      min={1}
                      max={16}
                      step={1}
                      value={field.state.value}
                      disabled={isDisabled || !speakers.enabled}
                      className="w-full text-right tabular-nums md:w-[132px]"
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingsRow>
                )}
              </form.Field>

              <form.Field name="speakers.windowSecs">
                {(field) => (
                  <SettingsRow
                    labelFor="diarization-window"
                    title={t(($) => $.config.window)}
                    description={t(($) => $.config.windowHint)}
                  >
                    <Input
                      id="diarization-window"
                      type="number"
                      min={5}
                      max={60}
                      step={1}
                      value={field.state.value}
                      disabled={isDisabled || !speakers.enabled || !speakers.diarizationEnabled}
                      className="w-full text-right tabular-nums md:w-[132px]"
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingsRow>
                )}
              </form.Field>

              <form.Field name="speakers.similarityThreshold">
                {(field) => (
                  <SettingsRow
                    labelFor="speaker-threshold"
                    title={t(($) => $.config.similarity)}
                    description={t(($) => $.config.similarityHint)}
                  >
                    <Input
                      id="speaker-threshold"
                      type="number"
                      min={0.3}
                      max={0.9}
                      step={0.05}
                      value={field.state.value}
                      disabled={isDisabled || !speakers.enabled}
                      className="w-full text-right tabular-nums md:w-[132px]"
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingsRow>
                )}
              </form.Field>
            </SettingsList>
          </SettingsSection>

          <details className="group overflow-hidden rounded-xl border bg-card">
            <summary className="flex cursor-default list-none items-center justify-between gap-3 px-4 py-3 text-[13px] font-medium [&::-webkit-details-marker]:hidden">
              <span>{t(($) => $.config.advancedSection)}</span>
              <Icon
                icon={chevronRightIcon}
                className="size-4 text-muted-foreground transition-transform group-open:rotate-90"
              />
            </summary>
            <div className="border-t border-border/70">
              <form.Field name="recognition.maxSegmentSecs">
                {(field) => (
                  <SettingsRow
                    labelFor="max-segment"
                    title={t(($) => $.config.maxSegment)}
                    description={t(($) => $.config.maxSegmentHint)}
                  >
                    <Input
                      id="max-segment"
                      type="number"
                      min={1}
                      max={30}
                      step={0.5}
                      value={field.state.value}
                      disabled={isDisabled}
                      className="w-full text-right tabular-nums md:w-[132px]"
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingsRow>
                )}
              </form.Field>

              <form.Field name="recognition.stableWaitMs">
                {(field) => (
                  <SettingsRow
                    labelFor="stable-wait"
                    title={t(($) => $.config.stableWait)}
                    description={t(($) => $.config.stableWaitHint)}
                  >
                    <Input
                      id="stable-wait"
                      type="number"
                      min={200}
                      max={10_000}
                      step={100}
                      value={field.state.value}
                      disabled={isDisabled}
                      className="w-full text-right tabular-nums md:w-[132px]"
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingsRow>
                )}
              </form.Field>

              <form.Field name="recognition.vadThreshold">
                {(field) => (
                  <SettingsRow labelFor="vad-threshold" title={t(($) => $.config.vadThreshold)}>
                    <Input
                      id="vad-threshold"
                      type="number"
                      min={0.05}
                      max={0.95}
                      step={0.05}
                      value={field.state.value}
                      disabled={isDisabled || readiness.selectedMode !== 'segmented'}
                      className="w-full text-right tabular-nums md:w-[132px]"
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingsRow>
                )}
              </form.Field>

              <form.Field name="recognition.minSilenceSecs">
                {(field) => (
                  <SettingsRow labelFor="min-silence" title={t(($) => $.config.minSilence)}>
                    <Input
                      id="min-silence"
                      type="number"
                      min={0.1}
                      max={2}
                      step={0.05}
                      value={field.state.value}
                      disabled={isDisabled || readiness.selectedMode !== 'segmented'}
                      className="w-full text-right tabular-nums md:w-[132px]"
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingsRow>
                )}
              </form.Field>

              <form.Field name="recognition.minSpeechSecs">
                {(field) => (
                  <SettingsRow labelFor="min-speech" title={t(($) => $.config.minSpeech)}>
                    <Input
                      id="min-speech"
                      type="number"
                      min={0.1}
                      max={2}
                      step={0.05}
                      value={field.state.value}
                      disabled={isDisabled || readiness.selectedMode !== 'segmented'}
                      className="w-full text-right tabular-nums md:w-[132px]"
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingsRow>
                )}
              </form.Field>

              <form.Field name="ui.historyLimit">
                {(field) => (
                  <SettingsRow
                    labelFor="history-limit"
                    title={t(($) => $.config.historyLimit)}
                    description={t(($) => $.config.historyLimitHint)}
                  >
                    <Input
                      id="history-limit"
                      type="number"
                      min={50}
                      max={2_000}
                      step={50}
                      value={field.state.value}
                      disabled={isDisabled}
                      className="w-full text-right tabular-nums md:w-[132px]"
                      onChange={(event) =>
                        field.handleChange(parseNumber(event.currentTarget.value))
                      }
                    />
                  </SettingsRow>
                )}
              </form.Field>
            </div>
          </details>

          <SettingsList>
            <div className="px-4 py-3">
              <p className="text-[11px] text-muted-foreground">
                {t(($) => $.config.modelsDirectory, { path: capabilities.modelsDir })}
              </p>
            </div>
          </SettingsList>
        </div>

        <div className="sticky bottom-0 z-10 mt-5 -mx-4 flex items-center justify-between gap-4 border-t bg-background/95 px-4 py-3 backdrop-blur md:-mx-6 md:px-6">
          <div className="min-w-0 text-xs">
            {isDirty && <span className="text-warning">{t(($) => $.config.unsaved)}</span>}
            {!isDirty && saveConfig.isSuccess && (
              <span className="flex items-center gap-1.5 text-success">
                <Icon icon={checkIcon} className="size-3.5" />
                {t(($) => $.config.saved)}
              </span>
            )}
          </div>
          <Button type="submit" disabled={isDisabled || !isDirty}>
            {saveConfig.isPending && <Icon icon={loaderCircleIcon} className="animate-spin" />}
            {saveLabel}
          </Button>
        </div>
      </form>

      <AlertDialog
        open={blocker.status === 'blocked'}
        onOpenChange={(isOpen) => {
          if (isOpen) return;
          if (blocker.status === 'blocked') blocker.reset();
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t(($) => $.config.leaveTitle)}</AlertDialogTitle>
            <AlertDialogDescription>{t(($) => $.config.leaveDescription)}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t(($) => $.config.keepEditing)}</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => {
                if (blocker.status === 'blocked') blocker.proceed();
              }}
            >
              {t(($) => $.config.discardChanges)}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
