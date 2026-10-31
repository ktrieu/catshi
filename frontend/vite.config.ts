import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { tanstackRouter } from '@tanstack/router-plugin/vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    // Must come before react() so generated routes are picked up.
    tanstackRouter({ target: 'react', autoCodeSplitting: true }),
    react(),
    tailwindcss(),
  ],
  server: {
    host: '0.0.0.0',
    port: Number(process.env.FRONTEND_PORT) || 5173,
    proxy: {
      // The API routes have no /api prefix, so strip it when forwarding.
      '/api': {
        target:
          process.env.API_URL ||
          `http://localhost:${process.env.API_PORT || 3000}`,
        rewrite: (path) => path.replace(/^\/api/, ''),
      },
    },
  },
})
