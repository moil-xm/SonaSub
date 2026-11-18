import type { ReactNode } from 'react';

type SummaryRowProps = {
  children: ReactNode;
  label: string;
};

export function SummaryRow({ children, label }: SummaryRowProps) {
  return (
    <div className="grid grid-cols-[88px_minmax(0,1fr)] gap-2 px-4 py-2.5">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="min-w-0 text-foreground">{children}</dd>
    </div>
  );
}
