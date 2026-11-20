import { Icon } from '@iconify/react';
import plusIcon from '@iconify-icons/lucide/plus';
import xIcon from '@iconify-icons/lucide/x';
import { useTranslation } from '@sonasub/i18n';
import { Button } from '@sonasub/ui/components/button';
import { Popover, PopoverContent, PopoverTrigger } from '@sonasub/ui/components/popover';

import type { RecordDevice } from '@/lib/records';
import { audioDeviceKey } from '@/lib/session';
import type { AudioDeviceRef, AudioSourceKind } from '@/lib/session';

type RecordDevicePickerProps = {
  active?: AudioDeviceRef[];
  currentRecordId?: string;
  devices: RecordDevice[];
  disabled: boolean;
  onRemove: (device: AudioDeviceRef) => void;
  onSelect: (device: AudioDeviceRef) => void;
  selected: AudioDeviceRef[];
};

function statusDotClass(isActive: boolean) {
  if (isActive) return 'bg-success';
  return 'bg-muted-foreground/45';
}

export function RecordDevicePicker({
  active = [],
  currentRecordId,
  devices,
  disabled,
  onRemove,
  onSelect,
  selected,
}: RecordDevicePickerProps) {
  const { t } = useTranslation();
  const selectedKeys = new Set(selected.map(audioDeviceKey));
  const activeKeys = new Set(active.map(audioDeviceKey));
  const availableDevices = devices.filter((device) => !selectedKeys.has(audioDeviceKey(device)));
  const deviceGroups: Array<{ kind: AudioSourceKind; devices: RecordDevice[] }> = [
    {
      kind: 'systemOutput',
      devices: availableDevices.filter((device) => device.kind === 'systemOutput'),
    },
    {
      kind: 'microphone',
      devices: availableDevices.filter((device) => device.kind === 'microphone'),
    },
  ];

  function deviceKindLabel(kind: AudioSourceKind) {
    if (kind === 'microphone') return t(($) => $.config.microphone);
    return t(($) => $.config.systemOutput);
  }

  function findDevice(deviceRef: AudioDeviceRef) {
    return devices.find((device) => audioDeviceKey(device) === audioDeviceKey(deviceRef));
  }

  function deviceName(deviceRef: AudioDeviceRef) {
    return findDevice(deviceRef)?.name ?? deviceRef.id;
  }

  function occupiedByCurrent(device: RecordDevice) {
    return currentRecordId !== undefined && device.occupiedRecordId === currentRecordId;
  }

  function deviceStatus(device: RecordDevice) {
    if (!device.captureSupported) {
      return device.captureReason ?? t(($) => $.records.device.captureUnavailable);
    }
    if (occupiedByCurrent(device)) return t(($) => $.records.device.runningHere);
    if (device.occupiedRecordName) {
      return t(($) => $.records.device.occupied, { name: device.occupiedRecordName });
    }
    return t(($) => $.records.device.available);
  }

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {selected.map((deviceRef) => {
        const key = audioDeviceKey(deviceRef);
        return (
          <span
            key={key}
            className="inline-flex h-8 items-center gap-1.5 rounded-md border bg-background px-2.5 text-xs font-medium"
          >
            <span className={`size-[7px] rounded-full ${statusDotClass(activeKeys.has(key))}`} />
            <span className="max-w-[220px] truncate">{deviceName(deviceRef)}</span>
            <span className="text-[10px] font-normal text-muted-foreground">
              {deviceKindLabel(deviceRef.kind)}
            </span>
            {!disabled && (
              <button
                type="button"
                aria-label={t(($) => $.records.editor.removeDevice, {
                  name: deviceName(deviceRef),
                })}
                className="-mr-1 grid size-5 place-items-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                onClick={() => onRemove(deviceRef)}
              >
                <Icon icon={xIcon} className="size-3" />
              </button>
            )}
          </span>
        );
      })}

      {!disabled && (
        <Popover>
          <PopoverTrigger asChild>
            <Button type="button" variant="outline" size="sm" className="h-8 border-dashed">
              <Icon icon={plusIcon} />
              {t(($) => $.records.editor.addDevice)}
            </Button>
          </PopoverTrigger>
          <PopoverContent align="start" className="w-[380px] p-2">
            {availableDevices.length === 0 && (
              <p className="px-2 py-3 text-xs text-muted-foreground">
                {t(($) => $.records.editor.noAvailableDevices)}
              </p>
            )}
            <div className="max-h-72 space-y-2 overflow-y-auto">
              {deviceGroups.map((group) => {
                if (group.devices.length === 0) return null;
                return (
                  <div key={group.kind}>
                    <p className="px-2 pb-1 text-[10px] font-medium tracking-[0.05em] text-muted-foreground uppercase">
                      {deviceKindLabel(group.kind)}
                    </p>
                    <div className="space-y-0.5">
                      {group.devices.map((device) => {
                        const isOccupied = !device.isAvailable && !occupiedByCurrent(device);
                        const isDisabled = !device.captureSupported || isOccupied;
                        let statusDot = 'bg-muted-foreground/45';
                        if (device.captureSupported && !isOccupied) statusDot = 'bg-success';
                        return (
                          <button
                            key={audioDeviceKey(device)}
                            type="button"
                            disabled={isDisabled}
                            className="flex w-full items-start gap-2 rounded-md px-2 py-2 text-left text-xs transition-colors hover:bg-muted disabled:cursor-not-allowed disabled:opacity-70 disabled:hover:bg-transparent"
                            onClick={() => onSelect({ kind: device.kind, id: device.id })}
                          >
                            <span
                              className={`mt-1.5 size-[7px] shrink-0 rounded-full ${statusDot}`}
                            />
                            <span className="min-w-0 flex-1">
                              <span className="block truncate font-medium">{device.name}</span>
                              <span className="block text-[10.5px] leading-4 text-muted-foreground">
                                {deviceStatus(device)}
                              </span>
                            </span>
                            {device.isDefault && (
                              <span className="shrink-0 pt-0.5 text-[10px] text-muted-foreground">
                                {t(($) => $.records.device.defaultDevice)}
                              </span>
                            )}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                );
              })}
            </div>
          </PopoverContent>
        </Popover>
      )}
    </div>
  );
}
