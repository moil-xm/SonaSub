import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { Icon } from '@iconify/react';
import arrowRightIcon from '@iconify-icons/lucide/arrow-right';
import chevronRightIcon from '@iconify-icons/lucide/chevron-right';
import loaderCircleIcon from '@iconify-icons/lucide/loader-circle';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import userRoundIcon from '@iconify-icons/lucide/user-round';
import { useForm, useStore } from '@tanstack/react-form';
import { useTranslation } from '@sonasub/i18n';
import { Alert, AlertDescription, AlertTitle } from '@sonasub/ui/components/alert';
import { Button } from '@sonasub/ui/components/button';
import { Input } from '@sonasub/ui/components/input';
import { Select, SelectContent, SelectItem, SelectTrigger } from '@sonasub/ui/components/select';
import { Switch } from '@sonasub/ui/components/switch';
import { Link, useNavigate } from '@tanstack/react-router';

import { RecordDevicePicker } from '@/components/record-device-picker';
import { LanguageName } from '@/components/session-config-form';
import { getErrorMessage } from '@/lib/error';
import { useCreateRecord, useUpdateRecord } from '@/lib/records';
import type { AsrMode, TranslationTrigger } from '@/lib/session';
import { clearRecordEditorDraft, saveRecordEditorDraft } from '@/stores/record-editor-draft';

import { RecordEditorChecks } from './RecordEditorChecks';
import type { RecordEditorFormProps } from './RecordEditorPage.type';
import { RecordEditorSection } from './RecordEditorSection';
import { RecordEditorSummary } from './RecordEditorSummary';
import { SelectCard } from './SelectCard';

function parseNumber(value: string): number {
  return Number(value);
}

export function RecordEditorForm({
  capabilities,
  devices,
  draftKey,
  initialConfig,
  initialName,
  recordId,
}: RecordEditorFormProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const createRecord = useCreateRecord();
  const updateRecord = useUpdateRecord();
  const [name, setName] = useState(initialName);
  const form = useForm({
    defaultValues: initialConfig,
  });
  const formConfig = useStore(form.store, (state) => state.values);
  const selectedAsr = capabilities.asrModels.find(
    (model) => model.id === formConfig.recognition.modelId,
  );
  const selectedTranslation = capabilities.translationModels.find(
    (model) => model.id === formConfig.translation.modelId,
  );
  const isSaving = createRecord.isPending || updateRecord.isPending;
  const saveError = createRecord.error ?? updateRecord.error;
  let sourceLanguageValue = 'auto';
  if (selectedAsr?.supportsSourceLanguage) {
    sourceLanguageValue = formConfig.recognition.sourceLanguage;
  }

  useEffect(() => {
    saveRecordEditorDraft(draftKey, name, formConfig);
  }, [draftKey, name, formConfig]);

  const sourceLanguageOptions: Array<{ value: string; label: ReactNode }> = [
    { value: 'auto', label: t(($) => $.records.editor.autoDetect) },
  ];
  if (selectedAsr?.supportsSourceLanguage) {
    sourceLanguageOptions.push(
      ...selectedAsr.sourceLanguages.map((language) => ({
        value: language,
        label: <LanguageName code={language} />,
      })),
    );
  }
  const targetLanguageOptions = capabilities.translationTargets.map((language) => ({
    value: language.code,
    label: <LanguageName code={language.code} />,
  }));
  const recognitionOptions = capabilities.asrModels.map((model) => {
    let label: string = model.label;
    if (!model.available) label = `${label} · ${t(($) => $.config.missing)}`;
    return { value: model.id, label };
  });
  const translationOptions = capabilities.translationModels.map((model) => {
    let label: string = model.label;
    if (!model.available) label = `${label} · ${t(($) => $.config.missing)}`;
    return { value: model.id, label };
  });

  let asrStatus: 'ready' | 'missing' = 'missing';
  if (selectedAsr?.available) asrStatus = 'ready';
  let translationStatus: 'ready' | 'missing' = 'missing';
  if (selectedTranslation?.available) translationStatus = 'ready';
  let asrDetail: string = t(($) => $.records.editor.modelMissing);
  if (selectedAsr?.available) asrDetail = t(($) => $.records.editor.modelReady);
  let translationDetail: string = t(($) => $.records.editor.modelMissing);
  if (selectedTranslation?.available) {
    translationDetail = t(($) => $.records.editor.modelReady);
  }

  function rememberDraft() {
    saveRecordEditorDraft(draftKey, name, formConfig);
  }

  function handleAsrChange(modelId: string) {
    form.setFieldValue('recognition.modelId', modelId);
    form.setFieldValue('recognition.mode', 'auto' satisfies AsrMode);
    const model = capabilities.asrModels.find((candidate) => candidate.id === modelId);
    if (!model) return;
    const currentLanguage = form.state.values.recognition.sourceLanguage;
    if (
      currentLanguage !== 'auto' &&
      (!model.supportsSourceLanguage || !model.sourceLanguages.includes(currentLanguage))
    ) {
      form.setFieldValue('recognition.sourceLanguage', 'auto');
    }
  }

  function handleTargetLanguageChange(targetLanguage: string) {
    form.setFieldValue('translation.targetLanguage', targetLanguage);
    const model = capabilities.translationModels.find(
      (candidate) => candidate.targetLanguage === targetLanguage,
    );
    if (!model) return;
    form.setFieldValue('translation.modelId', model.id);
    form.setFieldValue('translation.engine', model.engine);
  }

  function handleTranslationModelChange(modelId: string) {
    const model = capabilities.translationModels.find((candidate) => candidate.id === modelId);
    if (!model) return;
    form.setFieldValue('translation.modelId', model.id);
    form.setFieldValue('translation.targetLanguage', model.targetLanguage);
    form.setFieldValue('translation.engine', model.engine);
  }

  function handleTranslationTriggerChange(value: string) {
    if (value === 'finalOnly' || value === 'stableSegment') {
      form.setFieldValue('translation.trigger', value satisfies TranslationTrigger);
    }
  }

  function translationTriggerLabel(trigger: TranslationTrigger) {
    if (trigger === 'finalOnly') return t(($) => $.records.editor.triggers.finalOnly);
    return t(($) => $.records.editor.triggers.stableSegment);
  }

  async function handleSave() {
    const input = { name, config: form.state.values };
    let saved;
    if (recordId) {
      saved = await updateRecord.mutateAsync({ recordId, input });
    } else {
      saved = await createRecord.mutateAsync(input);
    }
    clearRecordEditorDraft(draftKey);
    await navigate({ to: '/', search: { record: saved.summary.id } });
  }

  async function handleCancel() {
    clearRecordEditorDraft(draftKey);
    if (recordId) {
      await navigate({ to: '/', search: { record: recordId } });
      return;
    }
    await navigate({ to: '/' });
  }

  const isSaveDisabled =
    isSaving || formConfig.audio.deviceIds.length === 0 || !capabilities.audio.systemCapture;
  const sourceLanguageDetail =
    selectedAsr?.sourceLanguages.map((language) => (
      <LanguageName key={language} code={language} />
    )) ?? [];
  let saveLabel: string = t(($) => $.records.editor.save);
  if (isSaving) saveLabel = t(($) => $.records.editor.saving);

  return (
    <div className="h-full overflow-y-auto">
      <form
        className="mx-auto flex min-h-full w-full max-w-[1180px] flex-col px-4 pb-5 md:px-6"
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void handleSave();
        }}
      >
        <div className="flex items-start justify-between gap-4 py-4">
          <div className="min-w-0 flex-1">
            <label
              htmlFor="record-name"
              className="text-[10.5px] font-medium tracking-[0.04em] text-muted-foreground uppercase"
            >
              {t(($) => $.records.editor.nameLabel)}
            </label>
            <Input
              id="record-name"
              value={name}
              placeholder={t(($) => $.records.editor.namePlaceholder)}
              className="mt-1 h-11 w-full rounded-none border-x-0 border-t-0 border-b border-border/70 bg-transparent px-0 text-2xl font-semibold tracking-tight shadow-none focus-visible:border-foreground/40 focus-visible:ring-0"
              onChange={(event) => setName(event.currentTarget.value)}
            />
          </div>
          <span className="mt-2 rounded-full bg-muted px-2.5 py-1 text-[10.5px] text-muted-foreground">
            {t(($) => $.records.editor.unsaved)}
          </span>
        </div>

        <div className="flex min-h-0 flex-1 items-start gap-4">
          <div className="min-w-0 flex-1 space-y-3">
            {saveError && (
              <Alert variant="destructive">
                <Icon icon={triangleAlertIcon} />
                <AlertTitle>{t(($) => $.records.editor.saveError)}</AlertTitle>
                <AlertDescription>{getErrorMessage(saveError)}</AlertDescription>
              </Alert>
            )}

            <RecordEditorSection index={1} title={t(($) => $.records.editor.deviceSection)}>
              {devices.length === 0 && (
                <p className="text-xs text-muted-foreground">
                  {t(($) => $.records.editor.noDevices)}
                </p>
              )}
              <form.Field name="audio.deviceIds">
                {(field) => (
                  <>
                    <p className="mb-2 text-[10.5px] text-muted-foreground">
                      {t(($) => $.records.editor.deviceHint)}
                    </p>
                    <RecordDevicePicker
                      devices={devices}
                      selectedIds={field.state.value}
                      disabled={false}
                      onSelect={(deviceId) => field.handleChange([...field.state.value, deviceId])}
                      onRemove={(deviceId) =>
                        field.handleChange(field.state.value.filter((id) => id !== deviceId))
                      }
                    />
                  </>
                )}
              </form.Field>
            </RecordEditorSection>

            <RecordEditorSection index={2} title={t(($) => $.records.editor.languageSection)}>
              <div className="grid items-stretch gap-2 sm:grid-cols-[minmax(0,1fr)_28px_minmax(0,1fr)]">
                <form.Field name="recognition.sourceLanguage">
                  {(field) => (
                    <SelectCard
                      id="record-source-language"
                      label={t(($) => $.records.editor.speechLanguage)}
                      value={sourceLanguageValue}
                      valueLabel={<LanguageName code={sourceLanguageValue} />}
                      detail={sourceLanguageDetail}
                      disabled={!selectedAsr?.supportsSourceLanguage}
                      options={sourceLanguageOptions}
                      onChange={field.handleChange}
                    />
                  )}
                </form.Field>
                <div className="grid place-items-center text-muted-foreground">
                  <Icon icon={arrowRightIcon} className="size-4 rotate-90 sm:rotate-0" />
                </div>
                <form.Field name="translation.targetLanguage">
                  {(field) => (
                    <SelectCard
                      id="record-target-language"
                      label={t(($) => $.records.editor.subtitleLanguage)}
                      value={field.state.value}
                      valueLabel={<LanguageName code={field.state.value} />}
                      detail={selectedTranslation?.label ?? ''}
                      disabled={false}
                      options={targetLanguageOptions}
                      onChange={handleTargetLanguageChange}
                    />
                  )}
                </form.Field>
              </div>

              <div className="mt-2 grid items-stretch gap-2 sm:grid-cols-[minmax(0,1fr)_28px_minmax(0,1fr)]">
                <form.Field name="recognition.modelId">
                  {(field) => (
                    <SelectCard
                      id="record-asr-model"
                      label={t(($) => $.records.editor.recognitionModel)}
                      value={field.state.value}
                      valueLabel={selectedAsr?.label ?? field.state.value}
                      detail={asrDetail}
                      status={asrStatus}
                      disabled={false}
                      options={recognitionOptions}
                      onChange={handleAsrChange}
                    />
                  )}
                </form.Field>
                <div className="grid place-items-center text-muted-foreground">
                  <Icon icon={arrowRightIcon} className="size-4 rotate-90 sm:rotate-0" />
                </div>
                <form.Field name="translation.modelId">
                  {(field) => (
                    <SelectCard
                      id="record-translation-model"
                      label={t(($) => $.records.editor.translationModel)}
                      value={field.state.value}
                      valueLabel={selectedTranslation?.label ?? field.state.value}
                      detail={translationDetail}
                      status={translationStatus}
                      disabled={false}
                      options={translationOptions}
                      onChange={handleTranslationModelChange}
                    />
                  )}
                </form.Field>
              </div>

              <div className="mt-2 flex justify-end">
                <Button
                  type="button"
                  variant="link"
                  size="sm"
                  className="h-auto px-0 text-[11px]"
                  asChild
                >
                  <Link to="/models" onClick={rememberDraft}>
                    {t(($) => $.records.editor.manageModels)}
                    <Icon icon={arrowRightIcon} className="size-3" />
                  </Link>
                </Button>
              </div>
            </RecordEditorSection>

            <section className="flex items-center gap-3 rounded-xl border bg-card px-4 py-3">
              <span className="grid size-8 place-items-center rounded-lg bg-muted text-muted-foreground">
                <Icon icon={userRoundIcon} className="size-4" />
              </span>
              <span className="min-w-0 flex-1">
                <span className="block text-[13px] font-semibold">
                  {t(($) => $.records.editor.identifySpeakers)}
                </span>
                <span className="block text-[10.5px] text-muted-foreground">
                  {t(($) => $.records.editor.identifySpeakersHint)}
                </span>
              </span>
              <form.Field name="speakers.enabled">
                {(field) => (
                  <Switch
                    id="record-speakers-enabled"
                    checked={field.state.value}
                    onCheckedChange={field.handleChange}
                  />
                )}
              </form.Field>
            </section>

            <details className="group overflow-hidden rounded-xl border bg-card">
              <summary className="flex cursor-default list-none items-center gap-3 px-4 py-3 [&::-webkit-details-marker]:hidden">
                <span className="grid size-5 place-items-center rounded-full bg-muted font-mono text-[10px] font-semibold text-muted-foreground">
                  04
                </span>
                <span className="min-w-0 flex-1 text-[13px] font-semibold">
                  {t(($) => $.records.editor.advancedSection)}
                </span>
                <Icon
                  icon={chevronRightIcon}
                  className="size-4 text-muted-foreground transition-transform group-open:rotate-90"
                />
              </summary>
              <div className="grid gap-3 border-t border-border/70 p-4 sm:grid-cols-2">
                <form.Field name="translation.trigger">
                  {(field) => (
                    <label className="space-y-1.5 text-xs">
                      <span className="block text-muted-foreground">
                        {t(($) => $.records.editor.translationTrigger)}
                      </span>
                      <Select
                        value={field.state.value}
                        onValueChange={handleTranslationTriggerChange}
                      >
                        <SelectTrigger className="h-9 w-full">
                          <span>{translationTriggerLabel(field.state.value)}</span>
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="finalOnly">
                            {t(($) => $.records.editor.triggers.finalOnly)}
                          </SelectItem>
                          <SelectItem value="stableSegment">
                            {t(($) => $.records.editor.triggers.stableSegment)}
                          </SelectItem>
                        </SelectContent>
                      </Select>
                    </label>
                  )}
                </form.Field>

                <form.Field name="ui.historyLimit">
                  {(field) => (
                    <label className="space-y-1.5 text-xs">
                      <span className="block text-muted-foreground">
                        {t(($) => $.records.editor.historyLimit)}
                      </span>
                      <Input
                        type="number"
                        min={50}
                        max={2000}
                        step={50}
                        value={field.state.value}
                        onChange={(event) =>
                          field.handleChange(parseNumber(event.currentTarget.value))
                        }
                      />
                    </label>
                  )}
                </form.Field>

                <form.Field name="recognition.maxSegmentSecs">
                  {(field) => (
                    <label className="space-y-1.5 text-xs">
                      <span className="block text-muted-foreground">
                        {t(($) => $.records.editor.maxSegment)}
                      </span>
                      <Input
                        type="number"
                        min={1}
                        max={30}
                        step={0.5}
                        value={field.state.value}
                        onChange={(event) =>
                          field.handleChange(parseNumber(event.currentTarget.value))
                        }
                      />
                    </label>
                  )}
                </form.Field>

                <form.Field name="recognition.stableWaitMs">
                  {(field) => (
                    <label className="space-y-1.5 text-xs">
                      <span className="block text-muted-foreground">
                        {t(($) => $.records.editor.stableWait)}
                      </span>
                      <Input
                        type="number"
                        min={200}
                        max={10000}
                        step={100}
                        value={field.state.value}
                        onChange={(event) =>
                          field.handleChange(parseNumber(event.currentTarget.value))
                        }
                      />
                    </label>
                  )}
                </form.Field>

                {formConfig.speakers.enabled && (
                  <>
                    <form.Field name="speakers.windowSecs">
                      {(field) => (
                        <label className="space-y-1.5 text-xs">
                          <span className="block text-muted-foreground">
                            {t(($) => $.records.editor.speakerWindow)}
                          </span>
                          <Input
                            type="number"
                            min={5}
                            max={60}
                            step={1}
                            value={field.state.value}
                            onChange={(event) =>
                              field.handleChange(parseNumber(event.currentTarget.value))
                            }
                          />
                        </label>
                      )}
                    </form.Field>
                    <form.Field name="speakers.maxSpeakers">
                      {(field) => (
                        <label className="space-y-1.5 text-xs">
                          <span className="block text-muted-foreground">
                            {t(($) => $.records.editor.maxSpeakers)}
                          </span>
                          <Input
                            type="number"
                            min={1}
                            max={16}
                            step={1}
                            value={field.state.value}
                            onChange={(event) =>
                              field.handleChange(parseNumber(event.currentTarget.value))
                            }
                          />
                        </label>
                      )}
                    </form.Field>
                  </>
                )}
              </div>
            </details>

            <div className="space-y-2 pt-1">
              <p className="text-[10.5px] font-medium tracking-[0.04em] text-muted-foreground uppercase">
                {t(($) => $.records.editor.checksSection)}
              </p>
              <RecordEditorChecks
                capabilities={capabilities}
                config={formConfig}
                devices={devices}
              />
            </div>
          </div>

          <RecordEditorSummary
            capabilities={capabilities}
            config={formConfig}
            devices={devices}
            name={name}
          />
        </div>

        <div className="sticky bottom-0 z-10 mt-5 -mx-4 flex items-center justify-end gap-4 border-t bg-background/95 px-4 py-3 backdrop-blur md:-mx-6 md:px-6">
          <div className="flex shrink-0 items-center gap-2">
            <Button type="button" variant="ghost" size="sm" onClick={() => void handleCancel()}>
              {t(($) => $.records.editor.cancel)}
            </Button>
            <Button type="submit" size="sm" disabled={isSaveDisabled}>
              {isSaving && <Icon icon={loaderCircleIcon} className="animate-spin" />}
              {saveLabel}
            </Button>
          </div>
        </div>
      </form>
    </div>
  );
}
