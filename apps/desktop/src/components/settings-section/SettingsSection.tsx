import type { ReactNode } from 'react';

type SettingsSectionProps = {
  children: ReactNode;
  description?: string;
  title: string;
};

export function SettingsSection({ title, description, children }: SettingsSectionProps) {
  return (
    <section className="space-y-2">
      <div className="flex items-baseline justify-between gap-4 px-1">
        <h2 className="text-[11px] font-semibold tracking-[0.08em] text-muted-foreground uppercase">
          {title}
        </h2>
        {description && (
          <p className="text-right text-[11px] text-muted-foreground">{description}</p>
        )}
      </div>
      {children}
    </section>
  );
}
