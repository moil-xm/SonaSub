import type { ChangeEvent } from 'react';
import { isLanguagePreference, useTranslation } from '@sonasub/i18n';
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@sonasub/ui/components/card';

import { setLanguagePreference, useLanguagePreference } from './i18n';

function handleLanguageChange(event: ChangeEvent<HTMLSelectElement>) {
  const preference = event.currentTarget.value;
  if (isLanguagePreference(preference)) setLanguagePreference(preference);
}

export function Popup() {
  const { t } = useTranslation();
  const languagePreference = useLanguagePreference();

  return (
    <div className="w-80 bg-background p-3 text-foreground">
      <Card>
        <CardHeader>
          <CardTitle>SonaSub</CardTitle>
          <CardDescription>{t(($) => $.extension.description)}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-3 text-xs text-muted-foreground">
          <p>{t(($) => $.extension.roadmap)}</p>
          <label className="flex items-center justify-between gap-3">
            <span>{t(($) => $.language.label)}</span>
            <select
              className="h-8 rounded-md border border-input bg-background px-2 text-sm text-foreground shadow-xs outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]"
              value={languagePreference}
              onChange={handleLanguageChange}
            >
              <option value="system">{t(($) => $.language.followSystem)}</option>
              <option value="en-US">{t(($) => $.language.english)}</option>
              <option value="zh-CN">{t(($) => $.language.chinese)}</option>
            </select>
          </label>
        </CardContent>
      </Card>
    </div>
  );
}
