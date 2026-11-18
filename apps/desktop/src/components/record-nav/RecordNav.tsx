import { Icon } from '@iconify/react';
import plusIcon from '@iconify-icons/lucide/plus';
import { useTranslation } from '@sonasub/i18n';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';

import { recordsOptions } from '@/lib/records';
import type { RecordState, RecordSummary } from '@/lib/records';

import { RecordStateDot } from './RecordStateDot';

type RecordNavProps = {
  activeRecordId: string | null;
  isLiveActive: boolean;
};

function recordItemClass(isActive: boolean) {
  if (isActive) {
    return 'flex h-[26px] items-center gap-2 rounded-md bg-muted px-2 text-xs font-medium text-foreground';
  }
  return 'flex h-[26px] items-center gap-2 rounded-md px-2 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground';
}

function currentPage(isActive: boolean): 'page' | undefined {
  if (isActive) return 'page';
  return undefined;
}

export function RecordNav({ activeRecordId, isLiveActive }: RecordNavProps) {
  const { t } = useTranslation();
  const { data: records, isPending, error } = useQuery(recordsOptions);

  function statusLabel(state: RecordState) {
    return t(($) => $.records.status[state]);
  }

  function subtitle(record: RecordSummary) {
    if (record.activeDeviceIds.length > 0) {
      return t(($) => $.records.nav.activeDeviceCount, {
        count: record.activeDeviceIds.length,
      });
    }
    return statusLabel(record.state);
  }

  return (
    <div className="mt-1">
      <div className="group flex items-center gap-1">
        <Link
          to="/"
          aria-current={currentPage(isLiveActive)}
          className={`${recordItemClass(isLiveActive)} min-w-0 flex-1`}
        >
          <span className="truncate">{t(($) => $.navigation.live)}</span>
        </Link>
        <Link
          to="/records/new"
          aria-label={t(($) => $.records.nav.add)}
          title={t(($) => $.records.nav.add)}
          className="grid size-6 shrink-0 place-items-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        >
          <Icon icon={plusIcon} className="size-3.5" />
        </Link>
      </div>

      <div className="mt-1 ml-3 max-h-[260px] overflow-y-auto border-l border-border pl-2">
        {isPending && (
          <p className="px-2 py-1.5 text-[11px] text-muted-foreground">
            {t(($) => $.records.nav.loading)}
          </p>
        )}
        {!isPending && error && (
          <p className="px-2 py-1.5 text-[11px] text-destructive">
            {t(($) => $.records.nav.loadError)}
          </p>
        )}
        {!isPending && !error && records?.length === 0 && (
          <p className="px-2 py-1.5 text-[11px] text-muted-foreground">
            {t(($) => $.records.nav.empty)}
          </p>
        )}
        {records?.map((record) => {
          const isActive = record.id === activeRecordId;
          return (
            <Link
              key={record.id}
              to="/"
              search={{ record: record.id }}
              aria-current={currentPage(isActive)}
              className={recordItemClass(isActive)}
            >
              <span className="min-w-0 flex-1">
                <span className="block truncate">{record.name}</span>
                <span className="block truncate text-[10.5px] text-muted-foreground">
                  {subtitle(record)}
                </span>
              </span>
              <RecordStateDot
                state={record.state}
                hasActiveSource={record.hasActiveSource}
                label={statusLabel(record.state)}
              />
            </Link>
          );
        })}
      </div>
    </div>
  );
}
