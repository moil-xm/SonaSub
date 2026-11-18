import type { AppConfig } from '@/lib/session';

export type RecordEditorDraft = {
  key: string;
  name: string;
  config: AppConfig;
  updatedAtMs: number;
};
