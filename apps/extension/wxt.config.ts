import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'wxt';

export default defineConfig({
  modules: ['@wxt-dev/module-react'],
  manifest: {
    name: 'SonaSub',
    description: '开源实时字幕项目的浏览器扩展入口。',
    permissions: [],
  },
  vite: () => ({
    plugins: [tailwindcss()],
  }),
});
