import { Icon } from '@iconify/react';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import { useTranslation } from '@sonasub/i18n';
import { Alert, AlertDescription, AlertTitle } from '@sonasub/ui/components/alert';

import { getErrorMessage } from '@/lib/error';

type SessionDataErrorsProps = {
  capabilitiesError: Error | null;
  configError: Error | null;
  snapshotError: Error | null;
};

export function SessionDataErrors({
  capabilitiesError,
  configError,
  snapshotError,
}: SessionDataErrorsProps) {
  const { t } = useTranslation();

  return (
    <>
      {capabilitiesError && (
        <Alert variant="destructive">
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.workbench.errors.capabilities)}</AlertTitle>
          <AlertDescription>{getErrorMessage(capabilitiesError)}</AlertDescription>
        </Alert>
      )}

      {configError && (
        <Alert variant="destructive">
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.workbench.errors.config)}</AlertTitle>
          <AlertDescription>{getErrorMessage(configError)}</AlertDescription>
        </Alert>
      )}

      {snapshotError && (
        <Alert variant="destructive">
          <Icon icon={triangleAlertIcon} />
          <AlertTitle>{t(($) => $.workbench.errors.snapshot)}</AlertTitle>
          <AlertDescription>{getErrorMessage(snapshotError)}</AlertDescription>
        </Alert>
      )}
    </>
  );
}
