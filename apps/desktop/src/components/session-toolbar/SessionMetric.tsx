type SessionMetricProps = {
  label: string;
  value: string;
};

export function SessionMetric({ label, value }: SessionMetricProps) {
  return (
    <div className="min-w-0 bg-card px-4 py-2.5">
      <dt className="text-[10.5px] font-medium tracking-[0.06em] text-muted-foreground uppercase">
        {label}
      </dt>
      <dd className="mt-0.5 truncate text-[12.5px] font-medium" title={value}>
        {value}
      </dd>
    </div>
  );
}
