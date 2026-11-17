import { AboutSettings } from './AboutSettings';
import { GeneralSettings } from './GeneralSettings';

type SettingsTab = 'general' | 'about';

type SettingsPageProps = {
  tab: SettingsTab;
};

export function SettingsPage({ tab }: SettingsPageProps) {
  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto w-full max-w-[1040px] px-4 py-4 md:px-6 md:py-5">
        {tab === 'general' && <GeneralSettings />}
        {tab === 'about' && <AboutSettings />}
      </div>
    </div>
  );
}
