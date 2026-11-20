import { useEffect } from 'react';
import { Icon } from '@iconify/react';
import captionsIcon from '@iconify-icons/lucide/captions';
import plusIcon from '@iconify-icons/lucide/plus';
import triangleAlertIcon from '@iconify-icons/lucide/triangle-alert';
import { useTranslation } from '@sonasub/i18n';
import { Alert, AlertDescription, AlertTitle } from '@sonasub/ui/components/alert';
import { Button } from '@sonasub/ui/components/button';
import { useQuery } from '@tanstack/react-query';
import { Link, useNavigate, useSearch } from '@tanstack/react-router';

import { RecordWorkspace } from '@/components/record-workspace';
import { getErrorMessage } from '@/lib/error';
import { recordDevicesOptions, recordSnapshotOptions, recordsOptions } from '@/lib/records';
import { sessionCapabilitiesOptions } from '@/lib/session';

export function OverviewPage() {
  const { t } = useTranslation();
  const { record: routeRecordId } = useSearch({ from: '/' });
  const navigate = useNavigate();
  const recordsQuery = useQuery(recordsOptions);
  const capabilitiesQuery = useQuery(sessionCapabilitiesOptions);
  const devicesQuery = useQuery(recordDevicesOptions);
  const records = recordsQuery.data ?? [];
  const activeRecord = records.findLast((record) => record.hasActiveSource);
  const latestRecord = records.at(-1);
  const fallbackRecordId = activeRecord?.id ?? latestRecord?.id;
  const routeRecord = routeRecordId
    ? records.find((record) => record.id === routeRecordId)
    : undefined;
  const selectedRecordId = routeRecord?.id ?? fallbackRecordId;
  const snapshotQuery = useQuery({
    ...recordSnapshotOptions(selectedRecordId ?? '__none__'),
    enabled: selectedRecordId !== undefined,
  });

  useEffect(() => {
    if (routeRecordId || !fallbackRecordId) return;
    void navigate({
      to: '/',
      search: { record: fallbackRecordId },
      replace: true,
    });
  }, [fallbackRecordId, navigate, routeRecordId]);

  const isInitialLoading =
    (!capabilitiesQuery.data && !capabilitiesQuery.error) ||
    (!devicesQuery.data && !devicesQuery.error) ||
    (!recordsQuery.data && !recordsQuery.error) ||
    (selectedRecordId !== undefined && !snapshotQuery.data && !snapshotQuery.error);

  if (isInitialLoading) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
        {t(($) => $.records.workspace.loading)}
      </div>
    );
  }

  const error =
    capabilitiesQuery.error ?? devicesQuery.error ?? recordsQuery.error ?? snapshotQuery.error;
  if (error) {
    return (
      <div className="h-full overflow-y-auto">
        <div className="mx-auto w-full max-w-[1040px] px-4 py-5 md:px-6">
          <Alert variant="destructive">
            <Icon icon={triangleAlertIcon} />
            <AlertTitle>{t(($) => $.records.workspace.errorTitle)}</AlertTitle>
            <AlertDescription>{getErrorMessage(error)}</AlertDescription>
          </Alert>
        </div>
      </div>
    );
  }

  if (records.length === 0) {
    return (
      <div className="flex h-full items-center justify-center px-6">
        <div className="flex max-w-md flex-col items-center text-center">
          <span className="grid size-12 place-items-center rounded-full bg-muted text-muted-foreground">
            <Icon icon={captionsIcon} className="size-5" />
          </span>
          <h1 className="mt-4 text-base font-semibold">
            {t(($) => $.records.workspace.noRecordTitle)}
          </h1>
          <p className="mt-1.5 text-xs leading-5 text-muted-foreground">
            {t(($) => $.records.workspace.noRecordDescription)}
          </p>
          <Button className="mt-4" size="sm" asChild>
            <Link to="/records/new">
              <Icon icon={plusIcon} />
              {t(($) => $.records.workspace.createSession)}
            </Link>
          </Button>
        </div>
      </div>
    );
  }

  const snapshot = snapshotQuery.data;
  if (!snapshot || !capabilitiesQuery.data || !devicesQuery.data) {
    return (
      <div className="flex h-full items-center justify-center px-6 text-center">
        <div>
          <p className="text-sm font-medium">{t(($) => $.records.workspace.noRecordTitle)}</p>
          <p className="mt-1 text-xs text-muted-foreground">
            {t(($) => $.records.workspace.noRecordDescription)}
          </p>
          <Button className="mt-4" size="sm" asChild>
            <Link to="/records/new">
              <Icon icon={plusIcon} />
              {t(($) => $.records.workspace.createSession)}
            </Link>
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-hidden">
      <RecordWorkspace
        capabilities={capabilitiesQuery.data}
        devices={devicesQuery.data}
        snapshot={snapshot}
      />
    </div>
  );
}
