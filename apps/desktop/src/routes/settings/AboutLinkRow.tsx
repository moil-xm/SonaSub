import { Icon } from '@iconify/react';
import arrowUpRightIcon from '@iconify-icons/lucide/arrow-up-right';
import { useTranslation } from '@sonasub/i18n';
import { Button } from '@sonasub/ui/components/button';
import { openUrl } from '@tauri-apps/plugin-opener';

import { SettingsRow } from '@/components/settings-section';

type AboutLinkRowProps = {
  description: string;
  title: string;
  url: string;
};

export function AboutLinkRow({ title, description, url }: AboutLinkRowProps) {
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
