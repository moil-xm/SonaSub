import { invoke } from '@tauri-apps/api/core';
import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query';

import type { ProviderConfig, ProviderFormValue, ProviderView } from './providers.type';

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export const providersOptions = queryOptions({
  queryKey: ['providers'],
  queryFn: () => invoke<ProviderView>('list_providers_command'),
  networkMode: 'always',
  retry: false,
  refetchOnWindowFocus: true,
});

export function useSaveProvider() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationKey: ['providers', 'save'],
    networkMode: 'always',
    mutationFn: (provider: ProviderConfig) =>
      invoke<ProviderView>('save_provider_command', { provider }),
    onSuccess: (view) => queryClient.setQueryData(providersOptions.queryKey, view),
  });
}

export function useFetchProviderModels() {
  return useMutation({
    mutationKey: ['providers', 'fetch-models'],
    networkMode: 'always',
    mutationFn: ({ baseUrl, apiKey }: { baseUrl: string; apiKey: string }) =>
      invoke<string[]>('fetch_provider_models', { baseUrl, apiKey }),
  });
}

export function useTestProvider() {
  return useMutation({
    mutationKey: ['providers', 'test'],
    networkMode: 'always',
    mutationFn: ({ baseUrl, apiKey, model }: { baseUrl: string; apiKey: string; model: string }) =>
      invoke<string>('test_provider_command', { baseUrl, apiKey, model }),
  });
}

export function buildProviderConfig(value: ProviderFormValue): ProviderConfig {
  return {
    name: value.name.trim(),
    baseUrl: value.baseUrl.trim(),
    apiKey: value.apiKey.trim(),
    icon: {
      name: value.iconName.trim(),
      dataUrl: value.iconDataUrl.trim(),
    },
    models: value.selectedModels.map((model) => model.trim()).filter(Boolean),
    concurrency: value.concurrency,
  };
}

export { errorText };
