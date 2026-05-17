import { defineConfig } from 'vitest/config'
import solidPlugin from 'vite-plugin-solid'

export default defineConfig({
  plugins: [solidPlugin()],
  test: {
    environment: 'jsdom',
    globals: false,
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    deps: {
      optimizer: { web: { include: ['solid-js'] } },
    },
    resolve: { conditions: ['development', 'browser'] },
  },
})
