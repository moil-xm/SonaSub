import type { ReactNode } from 'react';

type RecordMetricProps = {
  label: string;
  value: ReactNode;
};

export function RecordMetric({ label, value }: RecordMetricProps) {
  return (
    <div className="min-w-0 bg-card px-4 py-2.5">
      <dt className="text-[10px] font-medium tracking-[0.04em] text-muted-foreground uppercase">
        {label}
      </dt>
      <dd className="mt-1 truncate text-[11.5px] font-medium text-foreground">{value}</dd>
    </div>
  );
}
