import { describe, expect, it } from 'vite-plus/test';

import { buildProviderConfig } from '../providers';

describe('buildProviderConfig', () => {
  it('keeps a provider with no selected models saveable', () => {
    const provider = buildProviderConfig({
      name: 'OpenAI',
      baseUrl: 'https://api.openai.com/v1',
      apiKey: '',
      iconName: '',
      iconDataUrl: '',
      selectedModels: [],
      concurrency: 1,
    });

    expect(provider.models).toEqual([]);
    expect(provider.concurrency).toBe(1);
  });
});
