import { Icon } from '@iconify/react';
import checkIcon from '@iconify-icons/lucide/check';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';

type RecordCheckRowProps = {
  detail: string;
  isReady: boolean;
  label: string;
};

export function RecordCheckRow({ detail, isReady, label }: RecordCheckRowProps) {
  let statusClass = 'bg-warning/15 text-warning';
  let statusIcon = triangleAlertIcon;
  if (isReady) {
    statusClass = 'bg-success/15 text-success';
    statusIcon = checkIcon;
  }

  return (
    <div className="flex min-w-0 items-start gap-2.5 px-3 py-3">
      <span
        className={`mt-0.5 grid size-4 shrink-0 place-items-center rounded-full ${statusClass}`}
      >
        <Icon icon={statusIcon} className="size-2.5" />
      </span>
      <span className="min-w-0">
        <span className="block truncate text-xs font-medium">{label}</span>
        <span className="mt-0.5 block truncate text-[10.5px] text-muted-foreground">{detail}</span>
      </span>
    </div>
  );
}
