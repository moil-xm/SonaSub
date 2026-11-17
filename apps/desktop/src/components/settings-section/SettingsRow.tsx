import type { ReactNode } from 'react';
import { Label } from '@sonasub/ui/components/label';
import { cn } from '@sonasub/ui/lib/utils';

type SettingsRowProps = {
  children: ReactNode;
  className?: string;
  description?: string;
  labelFor?: string;
  title: string;
};

export function SettingsRow({
  title,
  description,
  labelFor,
  className,
  children,
}: SettingsRowProps) {
  return (
    <div
      className={cn(
        'flex flex-col gap-2 border-t border-border/70 px-4 py-3 first:border-t-0 md:flex-row md:items-center md:justify-between md:gap-6',
        className,
      )}
    >
      <div className="min-w-0 md:max-w-[56%]">
        {labelFor && (
          <Label htmlFor={labelFor} className="text-[13px] leading-5 font-medium">
            {title}
          </Label>
        )}
        {!labelFor && <p className="text-[13px] leading-5 font-medium">{title}</p>}
        {description && (
          <p className="mt-0.5 text-xs leading-5 text-muted-foreground">{description}</p>
        )}
      </div>
      <div className="flex w-full min-w-0 items-center justify-start gap-2 md:w-auto md:max-w-[52%] md:justify-end">
        {children}
      </div>
    </div>
  );
}
