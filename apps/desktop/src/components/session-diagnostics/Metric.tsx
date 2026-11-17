type MetricProps = {
  label: string;
  value: string;
};

export function Metric({ label, value }: MetricProps) {
  return (
    <div className="bg-card px-4 py-3">
      <p className="text-[10.5px] font-medium tracking-[0.06em] text-muted-foreground uppercase">
        {label}
      </p>
      <p className="mt-1 font-mono text-[15px] font-medium tabular-nums">{value}</p>
    </div>
  );
}
