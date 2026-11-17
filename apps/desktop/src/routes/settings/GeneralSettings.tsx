import { isLanguagePreference, useTranslation } from '@sonasub/i18n';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@sonasub/ui/components/select';
import { Switch } from '@sonasub/ui/components/switch';
import { useSetTheme, useTheme } from '@sonasub/ui/theme';

import {
  SegmentedControl,
  SettingsList,
  SettingsRow,
  SettingsSection,
} from '@/components/settings-section';
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

type AccentOption = {
  label: string;
  swatchClass: string;
  value: AccentColor;
};

export function GeneralSettings() {
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
