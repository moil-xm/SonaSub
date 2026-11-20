import { Icon } from '@iconify/react';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import { useTranslation } from '@sonasub/i18n';
import { Alert, AlertDescription, AlertTitle } from '@sonasub/ui/components/alert';
import { useQuery } from '@tanstack/react-query';

import { getErrorMessage } from '@/lib/error';
import { providersOptions } from '@/lib/providers';
import { sessionCapabilitiesOptions, sessionConfigOptions } from '@/lib/session';

import { ConfigForm } from './ConfigForm';

export function SessionConfigForm() {
  const { t } = useTranslation();
  const { data: capabilities, error: capabilitiesError } = useQuery(sessionCapabilitiesOptions);
  const { data: configView, error: configError } = useQuery(sessionConfigOptions);
  const { data: providersView } = useQuery(providersOptions);

  const isInitialLoading = (!capabilities && !capabilitiesError) || (!configView && !configError);
  if (isInitialLoading) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        {t(($) => $.config.loading)}
      </div>
    );
  }

  if (!capabilities || !configView) {
    return (
      <div className="h-full overflow-y-auto">
        <div className="mx-auto w-full max-w-[1040px] space-y-3 px-4 py-4 md:px-6 md:py-5">
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
        </div>
      </div>
    );
  }

  return (
    <ConfigForm
      capabilities={capabilities}
      configView={configView}
      configError={configError}
      capabilitiesError={capabilitiesError}
      providers={providersView?.providers ?? []}
    />
  );
}
