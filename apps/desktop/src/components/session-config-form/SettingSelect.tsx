import type { ReactNode } from 'react';
import { Select, SelectContent, SelectTrigger, SelectValue } from '@sonasub/ui/components/select';

type SettingSelectProps = {
  children: ReactNode;
  disabled: boolean;
  id: string;
  onChange: (value: string) => void;
  value: string;
};

export function SettingSelect({ id, value, disabled, onChange, children }: SettingSelectProps) {
  return (
    <Select value={value} onValueChange={onChange} disabled={disabled}>
      <SelectTrigger id={id} className="w-full md:w-[220px]">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>{children}</SelectContent>
    </Select>
  );
}
