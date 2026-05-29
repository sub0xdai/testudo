/** @anchor ui:journal:vitest.unit.config
 * @tags ui */

import { defineConfig } from 'vitest/config'

/** Minimal vitest config for pure-TS unit tests (no Solid.js plugin needed). */
export default defineConfig({
  test: {
    environment: 'jsdom',
    globals: false,
    include: ['src/**/*.test.ts'],
  },
})
