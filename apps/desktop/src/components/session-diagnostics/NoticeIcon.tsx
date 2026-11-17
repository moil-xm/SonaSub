import { Icon } from '@iconify/react';
import alertCircleIcon from '@iconify-icons/lucide/alert-circle';
import infoIcon from '@iconify-icons/lucide/info';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';

import type { SessionNotice } from '@/lib/session.type';

type NoticeIconProps = {
  notice: SessionNotice;
};

export function NoticeIcon({ notice }: NoticeIconProps) {
  if (notice.level === 'error') {
    return <Icon icon={alertCircleIcon} className="size-3.5 text-destructive" />;
  }
  if (notice.level === 'warning') {
    return <Icon icon={triangleAlertIcon} className="size-3.5 text-warning" />;
  }
  return <Icon icon={infoIcon} className="size-3.5 text-muted-foreground" />;
}
