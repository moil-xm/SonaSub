import { Icon } from '@iconify/react';
import downloadIcon from '@iconify-icons/lucide/download';
import externalLinkIcon from '@iconify-icons/lucide/external-link';
import folderIcon from '@iconify-icons/lucide/folder';
import folderOpenIcon from '@iconify-icons/lucide/folder-open';
import importIcon from '@iconify-icons/lucide/file-input';
import { useTranslation } from '@sonasub/i18n';
import type { TranslationResource } from '@sonasub/i18n';
import { Badge } from '@sonasub/ui/components/badge';
import { Button } from '@sonasub/ui/components/button';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@sonasub/ui/components/card';
import { openUrl } from '@tauri-apps/plugin-opener';
import { open } from '@tauri-apps/plugin-dialog';
import { useQuery } from '@tanstack/react-query';

import {
  modelDownloadSettingsOptions,
  modelInstallsOptions,
  useDownloadModel,
  useImportModelFiles,
  useSetModelDownloadDirectory,
} from '@/lib/model-installs';
import type { ModelCategory, ModelInstallGroup } from '@/lib/model-installs';
import { sessionCapabilitiesOptions } from '@/lib/session';
import type { AsrModelCapability } from '@/lib/session';
import { LanguageName } from '@/components/session-config-form';

type ModelCopyKey = keyof TranslationResource['models'];

const MODEL_COPY: Record<string, ModelCopyKey> = {
  'zipformer-bilingual-streaming': 'streamingZipformer',
  'sense-voice-int8': 'senseVoice',
  'qwen3-asr-0.6b-int8': 'qwen3ASR06B',
  'qwen3-asr-1.7b-int8': 'qwen3ASR17B',
  'bergamot-en-zh-Hans': 'bergamotEnZh',
  'bergamot-zh-Hans-en': 'bergamotZhEn',
  'silero-vad': 'sileroVad',
  'campplus-zh-en': 'campplus',
  'pyannote-segmentation-3.0': 'pyannote',
};

function formatSize(bytes: number) {
  if (bytes >= 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
  return `${Math.round(bytes / 1024 / 1024)} MB`;
}

function errorText(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function ModelCopy({ copyKey }: { copyKey: ModelCopyKey | undefined }) {
  const { t } = useTranslation();
  if (copyKey === 'streamingZipformer') {
    return (
      <div className="space-y-1.5">
        <p className="font-medium text-foreground">{t(($) => $.models.resourceNeeds)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.streamingZipformer.resource)}
        </p>
        <p className="font-medium text-foreground">{t(($) => $.models.effect)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.streamingZipformer.effect)}
        </p>
      </div>
    );
  }
  if (copyKey === 'senseVoice') {
    return (
      <div className="space-y-1.5">
        <p className="font-medium text-foreground">{t(($) => $.models.resourceNeeds)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.senseVoice.resource)}
        </p>
        <p className="font-medium text-foreground">{t(($) => $.models.effect)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.senseVoice.effect)}
        </p>
      </div>
    );
  }
  if (copyKey === 'qwen3ASR06B' || copyKey === 'qwen3ASR17B') {
    if (copyKey === 'qwen3ASR06B') {
      return (
        <div className="space-y-1.5">
          <p className="font-medium text-foreground">{t(($) => $.models.resourceNeeds)}</p>
          <p className="leading-relaxed text-muted-foreground">
            {t(($) => $.models.qwen3ASR06B.resource)}
          </p>
          <p className="font-medium text-foreground">{t(($) => $.models.effect)}</p>
          <p className="leading-relaxed text-muted-foreground">
            {t(($) => $.models.qwen3ASR06B.effect)}
          </p>
        </div>
      );
    }
    return (
      <div className="space-y-1.5">
        <p className="font-medium text-foreground">{t(($) => $.models.resourceNeeds)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.qwen3ASR17B.resource)}
        </p>
        <p className="font-medium text-foreground">{t(($) => $.models.effect)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.qwen3ASR17B.effect)}
        </p>
      </div>
    );
  }
  if (copyKey === 'sileroVad') {
    return (
      <div className="space-y-1.5">
        <p className="font-medium text-foreground">{t(($) => $.models.resourceNeeds)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.sileroVad.resource)}
        </p>
        <p className="font-medium text-foreground">{t(($) => $.models.effect)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.sileroVad.effect)}
        </p>
      </div>
    );
  }
  if (copyKey === 'campplus') {
    return (
      <div className="space-y-1.5">
        <p className="font-medium text-foreground">{t(($) => $.models.resourceNeeds)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.campplus.resource)}
        </p>
        <p className="font-medium text-foreground">{t(($) => $.models.effect)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.campplus.effect)}
        </p>
      </div>
    );
  }
  if (copyKey === 'pyannote') {
    return (
      <div className="space-y-1.5">
        <p className="font-medium text-foreground">{t(($) => $.models.resourceNeeds)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.pyannote.resource)}
        </p>
        <p className="font-medium text-foreground">{t(($) => $.models.effect)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.pyannote.effect)}
        </p>
      </div>
    );
  }
  if (copyKey === 'bergamotEnZh') {
    return (
      <div className="space-y-1.5">
        <p className="font-medium text-foreground">{t(($) => $.models.resourceNeeds)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.bergamotEnZh.resource)}
        </p>
        <p className="font-medium text-foreground">{t(($) => $.models.effect)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.bergamotEnZh.effect)}
        </p>
      </div>
    );
  }
  if (copyKey === 'bergamotZhEn') {
    return (
      <div className="space-y-1.5">
        <p className="font-medium text-foreground">{t(($) => $.models.resourceNeeds)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.bergamotZhEn.resource)}
        </p>
        <p className="font-medium text-foreground">{t(($) => $.models.effect)}</p>
        <p className="leading-relaxed text-muted-foreground">
          {t(($) => $.models.bergamotZhEn.effect)}
        </p>
      </div>
    );
  }
  return null;
}

type ModelCardProps = {
  group: ModelInstallGroup;
  capability?: AsrModelCapability;
};

function ModelCard({ group, capability }: ModelCardProps) {
  const { t } = useTranslation();
  const download = useDownloadModel();
  const importFiles = useImportModelFiles();
  const copyKey = MODEL_COPY[group.id];
  const missingFiles = group.files.filter((file) => !file.present);
  const presentFiles = group.files.filter((file) => file.present);
  const localBytes = group.files.reduce((total, file) => total + file.bytes, 0);
  const isReady = missingFiles.length === 0;
  const statusLabel = isReady
    ? t(($) => $.models.installed)
    : presentFiles.length > 0
      ? t(($) => $.models.filesReady, {
          present: presentFiles.length,
          total: group.files.length,
        })
      : t(($) => $.models.missing);
  const downloadLabel = isReady
    ? t(($) => $.models.reinstall)
    : t(($) => $.models.downloadAndInstall);

  async function handleImport() {
    const sources = await open({
      multiple: true,
      title: t(($) => $.models.importFiles),
    });
    if (!sources || sources.length === 0) return;
    await importFiles.mutateAsync({ groupId: group.id, sources });
  }

  async function handleImportDirectory() {
    const selected = await open({
      directory: true,
      multiple: true,
      title: t(($) => $.models.importDirectory),
    });
    if (!selected) return;
    await importFiles.mutateAsync({
      groupId: group.id,
      sources: Array.isArray(selected) ? selected : [selected],
    });
  }

  return (
    <Card className="gap-3 py-4">
      <CardHeader className="gap-1 px-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <CardTitle className="text-[14px]">{group.label}</CardTitle>
            <CardDescription className="text-xs">
              {group.version} ·{' '}
              {t(($) => $.models.downloadSize, { size: formatSize(group.downloadBytes) })}
            </CardDescription>
          </div>
          <Badge variant={isReady ? 'default' : 'secondary'}>{statusLabel}</Badge>
        </div>
      </CardHeader>
      <CardContent className="space-y-3 px-4 text-xs">
        <ModelCopy copyKey={copyKey} />
        {capability && (
          <div className="grid gap-1 rounded-md border bg-muted/30 p-2 text-muted-foreground sm:grid-cols-[auto_minmax(0,1fr)] sm:gap-x-3">
            <span className="font-medium text-foreground">
              {t(($) => $.models.recognitionMode)}
            </span>
            <span>
              {capability.kind === 'streaming'
                ? t(($) => $.config.asrModes.streaming)
                : t(($) => $.config.asrModes.segmented)}
            </span>
            <span className="font-medium text-foreground">
              {t(($) => $.models.supportedLanguages)}
            </span>
            <span className="break-words">
              {capability.sourceLanguages.map((language, index) => (
                <span key={language}>
                  {index > 0 && ' · '}
                  <LanguageName code={language} />
                </span>
              ))}
            </span>
            <span className="font-medium text-foreground">{t(($) => $.models.localUsage)}</span>
            <span>{formatSize(localBytes)}</span>
          </div>
        )}
        <p className="text-muted-foreground">{t(($) => $.models.importHint)}</p>
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            size="sm"
            disabled={download.isPending || importFiles.isPending}
            onClick={() => void download.mutateAsync(group.id)}
          >
            <Icon icon={downloadIcon} />
            {download.isPending ? t(($) => $.models.downloading) : downloadLabel}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={download.isPending || importFiles.isPending}
            onClick={() => void handleImport()}
          >
            <Icon icon={importIcon} />
            {importFiles.isPending ? t(($) => $.models.importing) : t(($) => $.models.importFiles)}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            disabled={download.isPending || importFiles.isPending}
            onClick={() => void handleImportDirectory()}
          >
            <Icon icon={folderIcon} />
            {importFiles.isPending
              ? t(($) => $.models.importing)
              : t(($) => $.models.importDirectory)}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="link"
            className="px-2"
            onClick={() => void openUrl(group.upstream)}
          >
            <Icon icon={externalLinkIcon} />
            {t(($) => $.models.openSource)}
          </Button>
        </div>
        {missingFiles.length > 0 && (
          <div>
            <p className="mb-1 font-medium text-foreground">{t(($) => $.models.requiredFiles)}</p>
            <ul className="space-y-1">
              {missingFiles.map((file) => (
                <li key={file.relative} className="flex items-center gap-2 text-muted-foreground">
                  <Icon icon={folderOpenIcon} className="size-3.5 shrink-0" />
                  <code className="min-w-0 truncate text-[11px]">{file.relative}</code>
                </li>
              ))}
            </ul>
          </div>
        )}
        {download.isError && (
          <p className="text-destructive">
            {t(($) => $.models.downloadError)}: {errorText(download.error)}
          </p>
        )}
        {importFiles.isError && (
          <p className="text-destructive">
            {t(($) => $.models.importError)}: {errorText(importFiles.error)}
          </p>
        )}
      </CardContent>
    </Card>
  );
}

export function ModelsPage() {
  const { t } = useTranslation();
  const { data: groups, isPending, isError } = useQuery(modelInstallsOptions);
  const { data: capabilities } = useQuery(sessionCapabilitiesOptions);
  const { data: settings } = useQuery(modelDownloadSettingsOptions);
  const setDirectory = useSetModelDownloadDirectory();
  const installedCount =
    groups?.filter((group) => group.files.every((file) => file.present)).length ?? 0;
  const totalCount = groups?.length ?? 0;
  const sections: { category: ModelCategory; title: string; description: string }[] = [
    {
      category: 'recognition',
      title: t(($) => $.models.recognitionSection),
      description: t(($) => $.models.recognitionSectionHint),
    },
    {
      category: 'translation',
      title: t(($) => $.models.translationSection),
      description: t(($) => $.models.translationSectionHint),
    },
    {
      category: 'supporting',
      title: t(($) => $.models.supportingSection),
      description: t(($) => $.models.supportingSectionHint),
    },
  ];

  async function handleChooseDirectory() {
    const directory = await open({ directory: true, multiple: false });
    if (typeof directory !== 'string') return;
    await setDirectory.mutateAsync(directory);
  }

  return (
    <div className="h-full overflow-hidden">
      <div className="mx-auto flex h-full w-full max-w-[1280px] flex-col gap-4 overflow-y-auto p-4 md:p-5">
        <div className="space-y-2">
          <div className="space-y-1">
            <h1 className="text-lg font-semibold">{t(($) => $.models.title)}</h1>
            <p className="text-muted-foreground">{t(($) => $.models.description)}</p>
            {groups && (
              <p className="text-xs text-muted-foreground">
                {t(($) => $.models.installedSummary, {
                  installed: installedCount,
                  total: totalCount,
                })}
              </p>
            )}
          </div>
          <div className="grid gap-3 rounded-lg border bg-muted/30 p-3 text-xs md:grid-cols-2">
            <div className="min-w-0 space-y-1">
              <p className="font-medium text-foreground">{t(($) => $.models.directory)}</p>
              <code className="block break-all text-muted-foreground">
                {settings?.modelsDir ?? '—'}
              </code>
              <p className="text-muted-foreground">{t(($) => $.models.installDirectoryHint)}</p>
            </div>
            <div className="min-w-0 space-y-2">
              <p className="font-medium text-foreground">{t(($) => $.models.downloadDirectory)}</p>
              <code className="block break-all text-muted-foreground">
                {settings?.directory ?? '—'}
              </code>
              <div className="flex flex-wrap items-center gap-2">
                <Button
                  type="button"
                  size="sm"
                  variant="outline"
                  disabled={setDirectory.isPending}
                  onClick={() => void handleChooseDirectory()}
                >
                  <Icon icon={folderIcon} />
                  {t(($) => $.models.chooseDownloadDirectory)}
                </Button>
                {settings && settings.directory !== settings.defaultDirectory && (
                  <Button
                    type="button"
                    size="sm"
                    variant="link"
                    disabled={setDirectory.isPending}
                    onClick={() => void setDirectory.mutateAsync(settings.defaultDirectory)}
                  >
                    {t(($) => $.models.resetDownloadDirectory)}
                  </Button>
                )}
              </div>
              <p className="text-muted-foreground">{t(($) => $.models.downloadDirectoryHint)}</p>
            </div>
          </div>
          {setDirectory.isError && (
            <p className="text-xs text-destructive">
              {t(($) => $.models.directoryError)}: {errorText(setDirectory.error)}
            </p>
          )}
        </div>
        {isPending && <p className="text-muted-foreground">{t(($) => $.models.loading)}</p>}
        {isError && <p className="text-destructive">{t(($) => $.models.loadError)}</p>}
        {sections.map((section) => {
          const sectionGroups = groups?.filter((group) => group.category === section.category);
          if (!sectionGroups || sectionGroups.length === 0) return null;
          return (
            <section key={section.category} className="space-y-2">
              <div>
                <h2 className="text-sm font-semibold">{section.title}</h2>
                <p className="text-xs text-muted-foreground">{section.description}</p>
              </div>
              <div className="grid gap-3 lg:grid-cols-2">
                {sectionGroups.map((group) => (
                  <ModelCard
                    key={group.id}
                    group={group}
                    capability={capabilities?.asrModels.find((model) => model.id === group.id)}
                  />
                ))}
              </div>
            </section>
          );
        })}
      </div>
    </div>
  );
}
