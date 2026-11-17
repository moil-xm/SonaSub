import type { ReactNode } from 'react';
import { cn } from '@sonasub/ui/lib/utils';

type SettingsListProps = {
  children: ReactNode;
  className?: string;
};

export function SettingsList({ children, className }: SettingsListProps) {
  return (
    <div className={cn('overflow-hidden rounded-xl border bg-card', className)}>{children}</div>
  );
}
