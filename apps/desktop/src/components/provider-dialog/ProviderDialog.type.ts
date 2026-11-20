import type { ProviderConfig } from '@/lib/providers';

export type ProviderDialogProps = {
  open: boolean;
  provider: ProviderConfig | null;
  onClose: () => void;
};
