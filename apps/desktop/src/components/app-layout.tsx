import type { ChangeEvent } from 'react';
import { Icon } from '@iconify/react';
import sunIcon from '@iconify-icons/lucide/sun';
import { isLanguagePreference, useTranslation } from '@sonasub/i18n';
import { Link, Outlet } from '@tanstack/react-router';
import { Button } from '@sonasub/ui/components/button';
import { useTheme, useToggleTheme } from '@sonasub/ui/theme';

import { setLanguagePreference, useLanguagePreference } from '@/i18n';

function handleLanguageChange(event: ChangeEvent<HTMLSelectElement>) {
  const preference = event.currentTarget.value;
  if (isLanguagePreference(preference)) setLanguagePreference(preference);
}

export function AppLayout() {
  const { t } = useTranslation();
  const theme = useTheme();
  const toggleTheme = useToggleTheme();
  const languagePreference = useLanguagePreference();
  let toggleLabel: string = t(($) => $.theme.switchToDark);
  if (theme === 'dark') toggleLabel = t(($) => $.theme.switchToLight);

  return (
    <div className="min-h-screen bg-background text-foreground">
      <header className="flex items-center justify-between border-b border-border px-6 py-4">
        <div className="flex items-center gap-6">
          <span className="text-sm font-semibold tracking-wide">SonaSub</span>
          <nav className="flex items-center gap-4 text-sm">
            <Link to="/" activeProps={{ className: 'text-primary font-medium' }}>
              {t(($) => $.navigation.overview)}
            </Link>
            <Link to="/about" activeProps={{ className: 'text-primary font-medium' }}>
              {t(($) => $.navigation.about)}
            </Link>
          </nav>
        </div>
        <div className="flex items-center gap-2">
          <select
            aria-label={t(($) => $.language.label)}
            className="h-8 rounded-md border border-input bg-background px-2 text-sm text-foreground shadow-xs outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]"
            value={languagePreference}
            onChange={handleLanguageChange}
          >
            <option value="system">{t(($) => $.language.followSystem)}</option>
            <option value="en-US">{t(($) => $.language.english)}</option>
            <option value="zh-CN">{t(($) => $.language.chinese)}</option>
          </select>
          <Button variant="outline" size="sm" onClick={toggleTheme}>
            <Icon icon={sunIcon} className="size-4" />
            {toggleLabel}
          </Button>
        </div>
      </header>
      <main className="mx-auto w-full max-w-3xl px-6 py-8">
        <Outlet />
      </main>
    </div>
  );
}
