import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 5173,
    proxy: {
      '/api/v1/clone': {
        target: 'http://localhost:8768',
        changeOrigin: true,
        rewrite: (path: string) => path.replace(/^\/api\/v1\/clone/, ''),
      },
      '/api': {
        target: 'http://localhost:3200',
        changeOrigin: true,
        ws: true,
      },
    },
  },
})
