import { Icon } from '@iconify/react';
import arrowUpRightIcon from '@iconify-icons/lucide/arrow-up-right';
import { getVersion } from '@tauri-apps/api/app';
import { isTauri } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { isLanguagePreference, useTranslation } from '@sonasub/i18n';
import { Button } from '@sonasub/ui/components/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@sonasub/ui/components/select';
import { Switch } from '@sonasub/ui/components/switch';
import { useSetTheme, useTheme } from '@sonasub/ui/theme';
import { useQuery } from '@tanstack/react-query';

import { setLanguagePreference, useLanguagePreference } from '@/i18n';
import {
  useAccentColor,
  useCaptionTextSize,
  useColorblindMode,
  useConfirmBeforeQuit,
  useReduceMotion,
  useSetAccentColor,
  useSetCaptionTextSize,
  useSetColorblindMode,
  useSetConfirmBeforeQuit,
  useSetReduceMotion,
} from '@/stores/app-preferences';
import type { AccentColor, CaptionTextSize } from '@/stores/app-preferences.type';

import {
  SegmentedControl,
  SettingsList,
  SettingsRow,
  SettingsSection,
} from '../components/settings-section';

const REPOSITORY_URL = 'https://github.com/moil-xm/SonaSub';
const RELEASES_URL = 'https://github.com/moil-xm/SonaSub/releases';
const LICENSE_URL = 'https://github.com/moil-xm/SonaSub/blob/main/LICENSE';
const ISSUES_URL = 'https://github.com/moil-xm/SonaSub/issues';

type SettingsTab = 'general' | 'about';

type SettingsPageProps = {
  tab: SettingsTab;
};

type AccentOption = {
  label: string;
  swatchClass: string;
  value: AccentColor;
};

function GeneralSettings() {
  const { t } = useTranslation();
  const theme = useTheme();
  const setTheme = useSetTheme();
  const accentColor = useAccentColor();
  const setAccentColor = useSetAccentColor();
  const captionTextSize = useCaptionTextSize();
  const setCaptionTextSize = useSetCaptionTextSize();
  const colorblindMode = useColorblindMode();
  const setColorblindMode = useSetColorblindMode();
  const reduceMotion = useReduceMotion();
  const setReduceMotion = useSetReduceMotion();
  const confirmBeforeQuit = useConfirmBeforeQuit();
  const setConfirmBeforeQuit = useSetConfirmBeforeQuit();
  const languagePreference = useLanguagePreference();

  const accentOptions: AccentOption[] = [
    {
      value: 'graphite',
      label: t(($) => $.settings.general.accents.graphite),
      swatchClass: 'bg-[var(--swatch-graphite)]',
    },
    {
      value: 'indigo',
      label: t(($) => $.settings.general.accents.indigo),
      swatchClass: 'bg-[var(--swatch-indigo)]',
    },
    {
      value: 'blue',
      label: t(($) => $.settings.general.accents.blue),
      swatchClass: 'bg-[var(--swatch-blue)]',
    },
    {
      value: 'teal',
      label: t(($) => $.settings.general.accents.teal),
      swatchClass: 'bg-[var(--swatch-teal)]',
    },
    {
      value: 'green',
      label: t(($) => $.settings.general.accents.green),
      swatchClass: 'bg-[var(--swatch-green)]',
    },
    {
      value: 'amber',
      label: t(($) => $.settings.general.accents.amber),
      swatchClass: 'bg-[var(--swatch-amber)]',
    },
    {
      value: 'rose',
      label: t(($) => $.settings.general.accents.rose),
      swatchClass: 'bg-[var(--swatch-rose)]',
    },
  ];
  const captionSizeOptions: { label: string; value: CaptionTextSize }[] = [
    {
      value: 'compact',
      label: t(($) => $.settings.general.captionSizes.compact),
    },
    {
      value: 'standard',
      label: t(($) => $.settings.general.captionSizes.standard),
    },
    {
      value: 'large',
      label: t(($) => $.settings.general.captionSizes.large),
    },
  ];

  return (
    <div className="space-y-6">
      <SettingsSection
        title={t(($) => $.settings.general.appearanceSection)}
        description={t(($) => $.settings.general.appearanceDescription)}
      >
        <SettingsList>
          <SettingsRow
            title={t(($) => $.settings.general.theme)}
            description={t(($) => $.settings.general.themeDescription)}
          >
            <SegmentedControl
              ariaLabel={t(($) => $.settings.general.theme)}
              value={theme}
              options={[
                { value: 'light', label: t(($) => $.settings.general.light) },
                { value: 'dark', label: t(($) => $.settings.general.dark) },
              ]}
              onChange={setTheme}
            />
          </SettingsRow>
          <SettingsRow
            title={t(($) => $.settings.general.accentColor)}
            description={t(($) => $.settings.general.accentDescription)}
          >
            <div
              role="group"
              aria-label={t(($) => $.settings.general.accentColor)}
              className="flex flex-wrap gap-2"
            >
              {accentOptions.map((option) => {
                let swatchClass =
                  'size-[18px] rounded-full ring-offset-2 ring-offset-card transition-shadow hover:ring-2 hover:ring-border';
                if (option.value === accentColor) {
                  swatchClass =
                    'size-[18px] rounded-full ring-2 ring-ring/60 ring-offset-2 ring-offset-card';
                }
                return (
                  <button
                    key={option.value}
                    type="button"
                    aria-label={option.label}
                    aria-pressed={option.value === accentColor}
                    title={option.label}
                    className={swatchClass}
                    onClick={() => setAccentColor(option.value)}
                  >
                    <span
                      className={`block size-full rounded-full border border-border/70 ${option.swatchClass}`}
                    />
                  </button>
                );
              })}
            </div>
          </SettingsRow>
          <SettingsRow
            title={t(($) => $.settings.general.captionSize)}
            description={t(($) => $.settings.general.captionSizeDescription)}
          >
            <SegmentedControl
              ariaLabel={t(($) => $.settings.general.captionSize)}
              value={captionTextSize}
              options={captionSizeOptions}
              onChange={setCaptionTextSize}
            />
          </SettingsRow>
          <SettingsRow
            title={t(($) => $.settings.general.colorblindMode)}
            description={t(($) => $.settings.general.colorblindDescription)}
            labelFor="settings-colorblind"
          >
            <Switch
              id="settings-colorblind"
              checked={colorblindMode}
              onCheckedChange={setColorblindMode}
            />
          </SettingsRow>
          <SettingsRow
            title={t(($) => $.settings.general.reduceMotion)}
            description={t(($) => $.settings.general.reduceMotionDescription)}
            labelFor="settings-reduce-motion"
          >
            <Switch
              id="settings-reduce-motion"
              checked={reduceMotion}
              onCheckedChange={setReduceMotion}
            />
          </SettingsRow>
        </SettingsList>
      </SettingsSection>

      <SettingsSection title={t(($) => $.settings.general.languageSection)}>
        <SettingsList>
          <SettingsRow
            title={t(($) => $.settings.general.interfaceLanguage)}
            description={t(($) => $.settings.general.interfaceLanguageDescription)}
            labelFor="settings-language"
          >
            <Select
              value={languagePreference}
              onValueChange={(value) => {
                if (isLanguagePreference(value)) setLanguagePreference(value);
              }}
            >
              <SelectTrigger id="settings-language" className="w-full md:w-[180px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="system">{t(($) => $.language.followSystem)}</SelectItem>
                <SelectItem value="en-US">{t(($) => $.language.english)}</SelectItem>
                <SelectItem value="zh-CN">{t(($) => $.language.chinese)}</SelectItem>
              </SelectContent>
            </Select>
          </SettingsRow>
        </SettingsList>
      </SettingsSection>

      <SettingsSection title={t(($) => $.settings.general.sessionSection)}>
        <SettingsList>
          <SettingsRow
            title={t(($) => $.settings.general.confirmQuit)}
            description={t(($) => $.settings.general.confirmQuitDescription)}
            labelFor="settings-confirm-quit"
          >
            <Switch
              id="settings-confirm-quit"
              checked={confirmBeforeQuit}
              onCheckedChange={setConfirmBeforeQuit}
            />
          </SettingsRow>
        </SettingsList>
      </SettingsSection>
    </div>
  );
}

type AboutLinkRowProps = {
  description: string;
  title: string;
  url: string;
};

function AboutLinkRow({ title, description, url }: AboutLinkRowProps) {
  const { t } = useTranslation();
  return (
    <SettingsRow title={title} description={description}>
      <Button
        variant="ghost"
        size="sm"
        aria-label={`${t(($) => $.settings.about.open)} ${title}`}
        onClick={() => void openUrl(url)}
      >
        {t(($) => $.settings.about.open)}
        <Icon icon={arrowUpRightIcon} />
      </Button>
    </SettingsRow>
  );
}

function AboutSettings() {
  const { t } = useTranslation();
  const versionQuery = useQuery({
    queryKey: ['app', 'version'],
    queryFn: getVersion,
    networkMode: 'always',
    retry: false,
    enabled: isTauri(),
  });
  let versionLabel = '…';
  if (versionQuery.data) versionLabel = versionQuery.data;
  else if (versionQuery.isError) versionLabel = t(($) => $.settings.about.versionUnavailable);

  return (
    <div className="space-y-6">
      <p className="max-w-2xl text-[13px] leading-5 text-muted-foreground">
        {t(($) => $.settings.about.description)}
      </p>

      <SettingsSection title={t(($) => $.settings.about.appSection)}>
        <SettingsList>
          <SettingsRow
            title={t(($) => $.settings.about.version)}
            description={t(($) => $.settings.about.versionDescription)}
          >
            <span className="font-mono text-xs tabular-nums text-muted-foreground">
              {versionLabel}
            </span>
          </SettingsRow>
        </SettingsList>
      </SettingsSection>

      <SettingsSection title={t(($) => $.settings.about.linksSection)}>
        <SettingsList>
          <AboutLinkRow
            title={t(($) => $.settings.about.repository)}
            description={t(($) => $.settings.about.repositoryDescription)}
            url={REPOSITORY_URL}
          />
          <AboutLinkRow
            title={t(($) => $.settings.about.releases)}
            description={t(($) => $.settings.about.releasesDescription)}
            url={RELEASES_URL}
          />
          <AboutLinkRow
            title={t(($) => $.settings.about.license)}
            description={t(($) => $.settings.about.licenseDescription)}
            url={LICENSE_URL}
          />
          <AboutLinkRow
            title={t(($) => $.settings.about.issues)}
            description={t(($) => $.settings.about.issuesDescription)}
            url={ISSUES_URL}
          />
        </SettingsList>
      </SettingsSection>

      <SettingsSection title={t(($) => $.settings.about.runtimeSection)}>
        <SettingsList>
          <p className="px-4 py-3 text-xs leading-5 text-muted-foreground">
            {t(($) => $.settings.about.runtimeDescription)}
          </p>
        </SettingsList>
      </SettingsSection>
    </div>
  );
}

function SettingsPage({ tab }: SettingsPageProps) {
  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto w-full max-w-[1040px] px-4 py-4 md:px-6 md:py-5">
        {tab === 'general' && <GeneralSettings />}
        {tab === 'about' && <AboutSettings />}
      </div>
    </div>
  );
}

export function GeneralSettingsPage() {
  return <SettingsPage tab="general" />;
}

export function AboutSettingsPage() {
  return <SettingsPage tab="about" />;
}
