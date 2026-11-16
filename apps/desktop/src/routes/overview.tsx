import { useTranslation } from '@sonasub/i18n';

import { EngineStatus } from '@/components/engine-status';

export function OverviewPage() {
  const { t } = useTranslation();

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-xl font-semibold">{t(($) => $.overview.title)}</h1>
        <p className="mt-1 text-sm text-muted-foreground">{t(($) => $.overview.description)}</p>
      </div>
      <EngineStatus />
    </div>
  );
}
