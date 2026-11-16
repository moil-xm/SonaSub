import { defineConfig } from 'vite-plus';

export default defineConfig({
  pack: {
    entry: ['src/main.ts'],
    format: ['esm'],
    platform: 'node',
    target: 'node24',
    dts: false,
    clean: true,
  },
});
