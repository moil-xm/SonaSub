import type { ReactNode } from 'react';
import { Select, SelectContent, SelectItem, SelectTrigger } from '@sonasub/ui/components/select';

type SelectCardOption = {
  disabled?: boolean;
  label: ReactNode;
  value: string;
};

type SelectCardProps = {
  detail: ReactNode;
  disabled: boolean;
  id: string;
  label: string;
  onChange: (value: string) => void;
  options: SelectCardOption[];
  status?: 'ready' | 'missing';
  value: string;
  valueLabel: ReactNode;
};

export function SelectCard({
  detail,
  disabled,
  id,
  label,
  onChange,
  options,
  status,
  value,
  valueLabel,
}: SelectCardProps) {
  let statusDot: ReactNode = null;
  if (status === 'ready') statusDot = <span className="size-1.5 rounded-full bg-success" />;
  if (status === 'missing') statusDot = <span className="size-1.5 rounded-full bg-warning" />;

  return (
    <Select value={value} onValueChange={onChange} disabled={disabled}>
      <SelectTrigger
        id={id}
        className="h-auto min-h-[74px] w-full items-center rounded-xl border bg-background px-3.5 py-3 text-left shadow-none transition-colors hover:border-foreground/20 disabled:opacity-60 [&_svg:last-child]:shrink-0"
      >
        <span className="flex min-w-0 flex-1 flex-col gap-1">
          <span className="text-[10.5px] font-medium tracking-[0.04em] text-muted-foreground uppercase">
            {label}
          </span>
          <span className="max-w-full truncate text-[14px] font-semibold text-foreground">
            {valueLabel}
          </span>
          <span className="flex min-w-0 items-center gap-1.5 truncate text-[11px] text-muted-foreground">
            {statusDot}
            <span className="truncate">{detail}</span>
          </span>
        </span>
      </SelectTrigger>
      <SelectContent position="popper" align="start" className="max-h-72">
        {options.map((option) => (
          <SelectItem key={option.value} value={option.value} disabled={option.disabled}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
