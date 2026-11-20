type RecordStateDotProps = {
  active: boolean;
  label: string;
};

export function RecordStateDot({ active, label }: RecordStateDotProps) {
  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      className={`size-[7px] shrink-0 rounded-full ${active ? 'bg-success' : 'bg-muted-foreground/45'}`}
    />
  );
}
