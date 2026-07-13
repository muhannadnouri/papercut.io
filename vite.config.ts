import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { resolve } from 'path'

export default defineConfig({
  plugins: [react()],
  server: {
    watch: {
      ignored: [
        '**/.cache/**',
        '**/.venv-silma/**',
        '**/sidecars/**/__pycache__/**',
        '**/sidecars/**/runtime/**',
        '**/src-tauri/target/**',
        '**/src-tauri/tts/runtime/**',
      ],
    },
  },
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
      },
    },
  },
  publicDir: 'public',
})
