import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const base = env.CZAR_WEB_BASE_PATH || '/';
  const apiProxyTarget = env.CZAR_WEB_API_PROXY_TARGET || 'http://localhost:4000';
  return {
    plugins: [react()],
    base,
    server: {
      host: '0.0.0.0',
      port: 5175,
      proxy: {
        '/api': {
          target: apiProxyTarget,
          changeOrigin: true,
        },
      },
    },
    build: {
      target: 'esnext',
    },
    preview: {
      proxy: {
        '/api': {
          target: apiProxyTarget,
          changeOrigin: true,
        },
      },
    },
  };
});
