import { Icon } from '@iconify/react';
import type { IconifyIcon } from '@iconify/react';
import loaderCircleIcon from '@iconify-icons/lucide/loader-circle';

type ActionIconProps = {
  icon: IconifyIcon;
  isLoading: boolean;
};

export function ActionIcon({ icon, isLoading }: ActionIconProps) {
  if (isLoading) return <Icon icon={loaderCircleIcon} className="animate-spin" />;
  return <Icon icon={icon} />;
}
