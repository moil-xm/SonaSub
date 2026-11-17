import { cn } from '@sonasub/ui/lib/utils';

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
