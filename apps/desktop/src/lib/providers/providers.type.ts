export type ProviderIcon = {
  name: string;
  dataUrl: string;
};

export type ProviderConfig = {
  name: string;
  baseUrl: string;
  apiKey: string;
  icon: ProviderIcon;
  models: string[];
  concurrency: number;
};

export type ProviderView = {
  providers: ProviderConfig[];
  loadError: string | null;
};

export type ProviderFormValue = {
  name: string;
  baseUrl: string;
  apiKey: string;
  iconName: string;
  iconDataUrl: string;
  selectedModels: string[];
  concurrency: number;
};
