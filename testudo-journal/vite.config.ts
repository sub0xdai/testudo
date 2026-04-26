import { defineConfig } from 'vite'
import solid from 'vite-plugin-solid'

export default defineConfig({
  plugins: [solid()],
  base: process.env.VITE_BASE_PATH || '/desk/',
  server: {
    port: 3002,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8080',
        changeOrigin: true,
      },
    },
  },
  build: {
    target: 'esnext',
    rollupOptions: {
      output: {
        manualChunks: {
          'vendor-wallet': [
            '@reown/appkit',
            '@reown/appkit-adapter-ethers',
            '@reown/appkit-adapter-solana',
            '@reown/appkit/networks',
            'ethers',
          ],
          'vendor-echarts': [
            'echarts',
            'echarts/core',
            'echarts/charts',
            'echarts/components',
            'echarts/renderers',
          ],
          'vendor-charts': [
            'lightweight-charts',
          ],
        },
      },
    },
  },
})
