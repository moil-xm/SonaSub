import type { RecordState } from '@/lib/records';

type RecordStateDotProps = {
  state: RecordState;
  hasActiveSource: boolean;
  label: string;
};

function tone(state: RecordState, hasActiveSource: boolean) {
  if (state === 'running' && hasActiveSource) return 'bg-success';
  return 'bg-muted-foreground/45';
}

export function RecordStateDot({ state, hasActiveSource, label }: RecordStateDotProps) {
  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      className={`size-[7px] shrink-0 rounded-full ${tone(state, hasActiveSource)}`}
    />
  );
}
