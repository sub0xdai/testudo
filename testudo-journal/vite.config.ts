import { defineConfig } from 'vite'
import solid from 'vite-plugin-solid'
import { visualizer } from 'rollup-plugin-visualizer'

export default defineConfig({
  plugins: [
    solid(),
    ...(process.env.ANALYZE === '1' ? [visualizer({ open: true, gzipSize: true, brotliSize: true })] : []),
  ],
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
    modulePreload: {
      polyfill: false,
      resolveDependencies: (_filename, deps) =>
        deps.filter((d) => !/vendor-wallet|vendor-echarts/.test(d)),
    },
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
