export type ModelInstallFile = {
  relative: string;
  expectedSha256: string | null;
  minBytes: number;
  sourceName: string;
  present: boolean;
  bytes: number;
};

export type ModelCategory = 'recognition' | 'translation' | 'supporting';

export type ModelInstallGroup = {
  id: string;
  label: string;
  version: string;
  category: ModelCategory;
  upstream: string;
  downloadKind: string;
  downloadUrl: string;
  downloadBytes: number;
  downloadUrls: string[];
  downloadBytesEach: number[];
  archiveSha256: string | null;
  files: ModelInstallFile[];
};

export type ModelDownloadSettings = {
  directory: string;
  defaultDirectory: string;
  modelsDir: string;
};
