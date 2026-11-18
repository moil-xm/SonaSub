import { Icon } from '@iconify/react';
import plusIcon from '@iconify-icons/lucide/plus';
import xIcon from '@iconify-icons/lucide/x';
import { useTranslation } from '@sonasub/i18n';
import { Button } from '@sonasub/ui/components/button';
import { Popover, PopoverContent, PopoverTrigger } from '@sonasub/ui/components/popover';

import type { RecordDevice } from '@/lib/records';

type RecordDevicePickerProps = {
  activeIds?: string[];
  currentRecordId?: string;
  devices: RecordDevice[];
  disabled: boolean;
  onRemove: (deviceId: string) => void;
  onSelect: (deviceId: string) => void;
  selectedIds: string[];
};

function statusDotClass(isActive: boolean) {
  if (isActive) return 'bg-success';
  return 'bg-muted-foreground/45';
}

export function RecordDevicePicker({
  activeIds = [],
  currentRecordId,
  devices,
  disabled,
  onRemove,
  onSelect,
  selectedIds,
}: RecordDevicePickerProps) {
  const { t } = useTranslation();
  const availableDevices = devices.filter((device) => !selectedIds.includes(device.id));

  function deviceName(deviceId: string) {
    const found = devices.find((device) => device.id === deviceId);
    return found?.name ?? deviceId;
  }

  function occupiedByCurrent(device: RecordDevice) {
    return currentRecordId !== undefined && device.occupiedRecordId === currentRecordId;
  }

  function deviceStatus(device: RecordDevice) {
    if (occupiedByCurrent(device)) return t(($) => $.records.device.runningHere);
    if (device.occupiedRecordName) {
      return t(($) => $.records.device.occupied, { name: device.occupiedRecordName });
    }
    return t(($) => $.records.device.available);
  }

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {selectedIds.map((deviceId) => (
        <span
          key={deviceId}
          className="inline-flex h-8 items-center gap-1.5 rounded-md border bg-background px-2.5 text-xs font-medium"
        >
          <span
            className={`size-[7px] rounded-full ${statusDotClass(activeIds.includes(deviceId))}`}
          />
          <span className="max-w-[220px] truncate">{deviceName(deviceId)}</span>
          {!disabled && (
            <button
              type="button"
              aria-label={t(($) => $.records.editor.removeDevice, { name: deviceName(deviceId) })}
              className="-mr-1 grid size-5 place-items-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              onClick={() => onRemove(deviceId)}
            >
              <Icon icon={xIcon} className="size-3" />
            </button>
          )}
        </span>
      ))}

      {!disabled && (
        <Popover>
          <PopoverTrigger asChild>
            <Button type="button" variant="outline" size="sm" className="h-8 border-dashed">
              <Icon icon={plusIcon} />
              {t(($) => $.records.editor.addDevice)}
            </Button>
          </PopoverTrigger>
          <PopoverContent align="start" className="w-[340px] p-2">
            {availableDevices.length === 0 && (
              <p className="px-2 py-3 text-xs text-muted-foreground">
                {t(($) => $.records.editor.noAvailableDevices)}
              </p>
            )}
            <div className="max-h-64 space-y-0.5 overflow-y-auto">
              {availableDevices.map((device) => {
                const isOccupied = !device.isAvailable && !occupiedByCurrent(device);
                return (
                  <button
                    key={device.id}
                    type="button"
                    disabled={isOccupied}
                    className="flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-xs transition-colors hover:bg-muted disabled:cursor-not-allowed disabled:opacity-70 disabled:hover:bg-transparent"
                    onClick={() => onSelect(device.id)}
                  >
                    <span
                      className={`size-[7px] shrink-0 rounded-full ${statusDotClass(isOccupied)}`}
                    />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate font-medium">{device.name}</span>
                      <span className="block truncate text-[10.5px] text-muted-foreground">
                        {deviceStatus(device)}
                      </span>
                    </span>
                    {device.isDefault && (
                      <span className="shrink-0 text-[10px] text-muted-foreground">
                        {t(($) => $.records.device.defaultDevice)}
                      </span>
                    )}
                  </button>
                );
              })}
            </div>
          </PopoverContent>
        </Popover>
      )}
    </div>
  );
}
