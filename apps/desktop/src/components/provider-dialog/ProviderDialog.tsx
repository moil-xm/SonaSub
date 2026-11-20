import { useRef, useState } from 'react';
import { Icon } from '@iconify/react';
import checkIcon from '@iconify-icons/lucide/check';
import loaderCircleIcon from '@iconify-icons/lucide/loader-circle';
import plusIcon from '@iconify-icons/lucide/plus';
import refreshIcon from '@iconify-icons/lucide/refresh-cw';
import uploadIcon from '@iconify-icons/lucide/image-up';
import xIcon from '@iconify-icons/lucide/x';
import { useTranslation } from '@sonasub/i18n';
import { Button } from '@sonasub/ui/components/button';
import { Input } from '@sonasub/ui/components/input';
import { Label } from '@sonasub/ui/components/label';

import {
  buildProviderConfig,
  errorText,
  useFetchProviderModels,
  useSaveProvider,
  useTestProvider,
} from '@/lib/providers';

import type { ProviderDialogProps } from './ProviderDialog.type';

type ModelTestState = 'testing' | 'passed' | 'failed';

const modelTestStateClass: Record<ModelTestState, string> = {
  testing: 'text-muted-foreground',
  passed: 'text-success',
  failed: 'text-destructive',
};

function readDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.addEventListener(
      'load',
      () => {
        if (typeof reader.result === 'string') resolve(reader.result);
        else reject(new Error('Icon file could not be read.'));
      },
      { once: true },
    );
    reader.addEventListener('error', () => reject(reader.error), { once: true });
    reader.readAsDataURL(file);
  });
}

export function ProviderDialog({ open, provider, onClose }: ProviderDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(provider?.name ?? '');
  const [baseUrl, setBaseUrl] = useState(provider?.baseUrl ?? '');
  const [apiKey, setApiKey] = useState(provider?.apiKey ?? '');
  const [iconName, setIconName] = useState(provider?.icon.name ?? '');
  const [iconDataUrl, setIconDataUrl] = useState(provider?.icon.dataUrl ?? '');
  const [models, setModels] = useState<string[]>(provider?.models ?? []);
  const [availableModels, setAvailableModels] = useState<string[]>(provider?.models ?? []);
  const [manualModel, setManualModel] = useState('');
  const [concurrency, setConcurrency] = useState(provider?.concurrency ?? 1);
  const [message, setMessage] = useState('');
  const [testStates, setTestStates] = useState<Record<string, ModelTestState>>({});
  const [isTesting, setIsTesting] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);
  const saveProvider = useSaveProvider();
  const fetchModels = useFetchProviderModels();
  const testProvider = useTestProvider();

  if (!open) return null;

  function addModel(model: string) {
    const trimmed = model.trim();
    if (!trimmed || models.includes(trimmed)) return;
    if (!availableModels.includes(trimmed)) {
      setAvailableModels([...availableModels, trimmed]);
    }
    setModels([...models, trimmed]);
  }

  function toggleModel(model: string) {
    setTestStates((current) => {
      const next = { ...current };
      delete next[model];
      return next;
    });
    if (models.includes(model)) {
      setModels(models.filter((current) => current !== model));
      return;
    }
    addModel(model);
  }

  async function handleFetchModels() {
    setMessage('');
    try {
      const fetched = await fetchModels.mutateAsync({ baseUrl, apiKey });
      const manualModels = availableModels.filter((model) => !fetched.includes(model));
      setAvailableModels([...fetched, ...manualModels]);
      setMessage('');
    } catch (error) {
      setMessage(t(($) => $.models.providers.fetchError));
      console.error(errorText(error));
    }
  }

  async function handleTest() {
    setMessage('');
    if (isTesting) return;
    if (models.length === 0) {
      setMessage(t(($) => $.models.providers.modelRequiredForTest));
      return;
    }
    const selectedModels = models;
    setIsTesting(true);
    setTestStates(Object.fromEntries(selectedModels.map((model) => [model, 'testing'])));
    void (async () => {
      const results = await Promise.all(
        selectedModels.map(async (model) => {
          try {
            const reply = await testProvider.mutateAsync({ baseUrl, apiKey, model });
            return { model, ok: true, reply };
          } catch (error) {
            return { model, ok: false, reply: errorText(error) };
          }
        }),
      );
      setTestStates(
        Object.fromEntries(
          results.map((result) => [result.model, result.ok ? 'passed' : 'failed']),
        ),
      );
      setIsTesting(false);
      const firstFailure = results.find((result) => !result.ok);
      if (firstFailure) {
        setMessage(`${t(($) => $.models.providers.testError)}: ${firstFailure.reply}`);
      } else {
        setMessage('');
      }
    })();
  }

  async function handleIconChange(file: File | undefined) {
    if (!file) return;
    setIconName(file.name);
    setIconDataUrl(await readDataUrl(file));
  }

  async function handleSave() {
    setMessage('');
    if (!name.trim()) {
      setMessage(t(($) => $.models.providers.nameRequired));
      return;
    }
    if (!baseUrl.trim()) {
      setMessage(t(($) => $.models.providers.baseUrlRequired));
      return;
    }
    const config = buildProviderConfig({
      name,
      baseUrl,
      apiKey,
      iconName,
      iconDataUrl,
      selectedModels: models,
      concurrency,
    });
    try {
      await saveProvider.mutateAsync(config);
      onClose();
    } catch (error) {
      setMessage(`${t(($) => $.models.providers.saveError)}: ${errorText(error)}`);
    }
  }

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/30 p-6">
      <div className="w-full max-w-[720px] overflow-hidden rounded-xl border bg-card shadow-lg">
        <div className="flex h-12 items-center justify-between border-b px-4">
          <h2 className="flex items-center gap-2 text-sm font-semibold">
            <Icon icon={plusIcon} className="size-4 text-primary" />
            {t(($) => $.models.providers.dialogAdd)}
          </h2>
          <Button type="button" variant="ghost" size="icon" onClick={onClose}>
            <Icon icon={xIcon} />
          </Button>
        </div>

        <div className="space-y-4 p-4 text-xs">
          <div className="space-y-1.5">
            <Label htmlFor="provider-name">{t(($) => $.models.providers.providerName)}</Label>
            <Input
              id="provider-name"
              value={name}
              placeholder={t(($) => $.models.providers.providerNamePlaceholder)}
              onChange={(event) => setName(event.target.value)}
            />
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="provider-base-url">{t(($) => $.models.providers.baseUrl)}</Label>
            <Input
              id="provider-base-url"
              value={baseUrl}
              placeholder={t(($) => $.models.providers.baseUrlPlaceholder)}
              onChange={(event) => setBaseUrl(event.target.value)}
            />
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="provider-api-key">{t(($) => $.models.providers.apiKey)}</Label>
            <Input
              id="provider-api-key"
              type="password"
              value={apiKey}
              placeholder={t(($) => $.models.providers.apiKeyPlaceholder)}
              onChange={(event) => setApiKey(event.target.value)}
            />
          </div>

          <div className="space-y-1.5">
            <Label>{t(($) => $.models.providers.icon)}</Label>
            <div className="flex items-center gap-2">
              <input
                ref={fileInput}
                type="file"
                accept="image/*"
                className="hidden"
                onChange={(event) => void handleIconChange(event.target.files?.[0])}
              />
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => fileInput.current?.click()}
              >
                <Icon icon={uploadIcon} />
                {t(($) => $.models.providers.chooseFile)}
              </Button>
              {iconDataUrl && (
                <img
                  src={iconDataUrl}
                  alt={iconName}
                  className="size-8 rounded-md border object-cover"
                />
              )}
              <span className="truncate text-muted-foreground">
                {iconName || t(($) => $.models.providers.noFile)}
              </span>
            </div>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <Label>{t(($) => $.models.providers.availableModels)}</Label>
              <span className="text-muted-foreground">
                {t(($) => $.models.providers.fetchedAgo)}
              </span>
            </div>
            <div className="flex gap-2">
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => setModels(availableModels)}
              >
                {t(($) => $.models.providers.selectAll)}
              </Button>
              <Button type="button" size="sm" variant="outline" onClick={() => setModels([])}>
                {t(($) => $.models.providers.selectNone)}
              </Button>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {availableModels.map((model) => {
                const selected = models.includes(model);
                const testState = testStates[model];
                return (
                  <button
                    key={model}
                    type="button"
                    className={`rounded-full border px-2.5 py-1 ${
                      selected
                        ? 'border-primary bg-primary/10 text-primary'
                        : 'bg-background text-muted-foreground'
                    }`}
                    onClick={() => toggleModel(model)}
                  >
                    {selected && <Icon icon={checkIcon} className="mr-1 inline size-3" />}
                    {model}
                    {testState && (
                      <span
                        data-testid={`model-status-${model}`}
                        data-state={testState}
                        className="ml-1.5 inline-flex size-3 items-center justify-center"
                      >
                        {testState === 'testing' ? (
                          <Icon
                            icon={loaderCircleIcon}
                            className={`size-3 animate-spin ${modelTestStateClass[testState]}`}
                          />
                        ) : (
                          <span
                            className={`size-2 rounded-full bg-current ${modelTestStateClass[testState]}`}
                          />
                        )}
                      </span>
                    )}
                  </button>
                );
              })}
              {availableModels.length === 0 && <span className="text-muted-foreground">—</span>}
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Input
                className="h-8 w-48"
                value={manualModel}
                placeholder={t(($) => $.models.providers.addModelId)}
                onChange={(event) => setManualModel(event.target.value)}
              />
              <Button
                type="button"
                size="sm"
                variant="outline"
                onClick={() => {
                  addModel(manualModel);
                  setManualModel('');
                }}
              >
                {t(($) => $.models.providers.add)}
              </Button>
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={fetchModels.isPending}
                onClick={() => void handleFetchModels()}
              >
                <Icon icon={refreshIcon} />
                {t(($) => $.models.providers.refreshModels)}
              </Button>
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={isTesting || models.length === 0}
                onClick={() => void handleTest()}
              >
                {isTesting && <Icon icon={loaderCircleIcon} className="animate-spin" />}
                {t(($) => $.models.providers.testModel)}
              </Button>
            </div>
          </div>

          <div className="space-y-2">
            <p className="text-xs font-semibold">{t(($) => $.models.providers.advanced)}</p>
            <div className="rounded-lg border bg-muted/20 p-3">
              <div className="flex items-center justify-between gap-4">
                <div>
                  <p className="font-medium">{t(($) => $.models.providers.concurrentRequests)}</p>
                  <p className="text-muted-foreground">
                    {t(($) => $.models.providers.concurrentRequestsHint)}
                  </p>
                </div>
                <Input
                  type="number"
                  min={1}
                  max={16}
                  className="h-8 w-20"
                  value={concurrency}
                  onChange={(event) => setConcurrency(Number(event.target.value))}
                />
              </div>
            </div>
          </div>

          {message && <p className="text-destructive">{message}</p>}
        </div>

        <div className="flex h-14 items-center justify-between border-t bg-muted/20 px-4">
          <span className="text-xs text-muted-foreground">
            {t(($) => $.models.providers.modelsCount, { count: models.length })}
          </span>
          <div className="flex gap-2">
            <Button type="button" variant="outline" onClick={onClose}>
              {t(($) => $.models.providers.cancel)}
            </Button>
            <Button
              type="button"
              disabled={saveProvider.isPending}
              onClick={() => void handleSave()}
            >
              {t(($) => $.models.providers.save)}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
