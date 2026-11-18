import { invoke } from '@tauri-apps/api/core';
import { queryOptions, useMutation, useQueryClient } from '@tanstack/react-query';

import { sessionCapabilitiesOptions } from '@/lib/session';

import type { ModelDownloadSettings, ModelInstallGroup } from './model-installs.type';

export const modelInstallsOptions = queryOptions({
  queryKey: ['models', 'installs'],
  queryFn: () => invoke<ModelInstallGroup[]>('list_model_installs'),
  networkMode: 'always',
  retry: false,
  staleTime: 15_000,
  refetchOnWindowFocus: true,
});

export const modelDownloadSettingsOptions = queryOptions({
  queryKey: ['models', 'download-settings'],
  queryFn: () => invoke<ModelDownloadSettings>('get_model_download_settings'),
  networkMode: 'always',
  retry: false,
  staleTime: 15_000,
  refetchOnWindowFocus: true,
});

function invalidateModelData(queryClient: ReturnType<typeof useQueryClient>) {
  void queryClient.invalidateQueries({ queryKey: modelInstallsOptions.queryKey });
  void queryClient.invalidateQueries({ queryKey: modelDownloadSettingsOptions.queryKey });
  void queryClient.invalidateQueries({ queryKey: sessionCapabilitiesOptions.queryKey });
}

export function useDownloadModel() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationKey: ['models', 'download'],
    scope: { id: 'model-download' },
    networkMode: 'always',
    mutationFn: (groupId: string) => invoke<void>('download_model', { groupId }),
    onSuccess: () => invalidateModelData(queryClient),
  });
}

export function useImportModelFiles() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationKey: ['models', 'import'],
    scope: { id: 'model-import' },
    networkMode: 'always',
    mutationFn: ({ groupId, sources }: { groupId: string; sources: string[] }) =>
      invoke<void>('import_model_files', { groupId, sources }),
    onSuccess: () => invalidateModelData(queryClient),
  });
}

export function useSetModelDownloadDirectory() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationKey: ['models', 'download-directory'],
    scope: { id: 'model-download-directory' },
    networkMode: 'always',
    mutationFn: (directory: string) =>
      invoke<ModelDownloadSettings>('set_model_download_directory', { directory }),
    onSuccess: () => invalidateModelData(queryClient),
  });
}
