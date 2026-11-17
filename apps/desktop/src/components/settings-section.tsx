import type { ReactNode } from 'react';
import { Label } from '@sonasub/ui/components/label';
import { cn } from '@sonasub/ui/lib/utils';

type SettingsSectionProps = {
  children: ReactNode;
  description?: string;
  title: string;
};

type SettingsListProps = {
  children: ReactNode;
  className?: string;
};

type SettingsRowProps = {
  children: ReactNode;
  className?: string;
  description?: string;
  labelFor?: string;
  title: string;
};

type SegmentedOption<T extends string> = {
  label: string;
  value: T;
};

type SegmentedControlProps<T extends string> = {
  ariaLabel: string;
  className?: string;
  onChange: (value: T) => void;
  options: readonly SegmentedOption<T>[];
  value: T;
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

export function SettingsList({ children, className }: SettingsListProps) {
  return (
    <div className={cn('overflow-hidden rounded-xl border bg-card', className)}>{children}</div>
  );
}

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

export function SegmentedControl<T extends string>({
  ariaLabel,
  className,
  options,
  value,
  onChange,
}: SegmentedControlProps<T>) {
  return (
    <div
      role="group"
      aria-label={ariaLabel}
      className={cn('inline-flex shrink-0 rounded-lg bg-muted p-0.5', className)}
    >
      {options.map((option) => {
        let optionClass =
          'h-6 rounded-md px-2.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground';
        if (option.value === value) {
          optionClass =
            'h-6 rounded-md bg-card px-2.5 text-xs font-medium text-foreground shadow-sm';
        }
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={option.value === value}
            className={optionClass}
            onClick={() => onChange(option.value)}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
