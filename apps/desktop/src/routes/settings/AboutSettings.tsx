import { getVersion } from '@tauri-apps/api/app';
import { isTauri } from '@tauri-apps/api/core';
import { useTranslation } from '@sonasub/i18n';
import { useQuery } from '@tanstack/react-query';

import { SettingsList, SettingsRow, SettingsSection } from '@/components/settings-section';

import { AboutLinkRow } from './AboutLinkRow';

const REPOSITORY_URL = 'https://github.com/moil-xm/SonaSub';
const RELEASES_URL = 'https://github.com/moil-xm/SonaSub/releases';
const LICENSE_URL = 'https://github.com/moil-xm/SonaSub/blob/main/LICENSE';
const ISSUES_URL = 'https://github.com/moil-xm/SonaSub/issues';

export function AboutSettings() {
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
