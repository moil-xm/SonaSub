import dayjs from 'dayjs';
import { useTranslation } from '@sonasub/i18n';

export function AboutPage() {
  const { t } = useTranslation();
  const modules = [
    { name: 'apps/desktop', role: t(($) => $.about.modules.desktop) },
    { name: 'apps/desktop/src-tauri', role: t(($) => $.about.modules.native) },
    { name: 'apps/extension', role: t(($) => $.about.modules.extension) },
    { name: 'packages/ui', role: t(($) => $.about.modules.ui) },
    { name: 'packages/i18n', role: t(($) => $.about.modules.i18n) },
  ];

  return (
    <div className="space-y-6">
      <p className="text-sm text-muted-foreground">
        {t(($) => $.about.description, { time: dayjs().format('YYYY-MM-DD HH:mm') })}
      </p>
      <h2 className="text-sm font-medium">
        {t(($) => $.about.moduleCount, { count: modules.length })}
      </h2>
      <ul className="divide-y divide-border rounded-lg border border-border text-sm">
        {modules.map(({ name, role }) => (
          <li key={name} className="flex items-baseline justify-between gap-4 px-4 py-3">
            <code className="font-mono text-xs">{name}</code>
            <span className="text-muted-foreground">{role}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
