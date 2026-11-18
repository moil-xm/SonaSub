import type { AppConfig, Capabilities } from '@/lib/session';
import type { RecordDevice } from '@/lib/records';

export type RecordEditorFormProps = {
  capabilities: Capabilities;
  devices: RecordDevice[];
  draftKey: string;
  initialConfig: AppConfig;
  initialName: string;
  recordId?: string;
};
