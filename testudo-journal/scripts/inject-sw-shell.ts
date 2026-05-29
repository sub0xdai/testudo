/** @anchor infra:journal-script:inject-sw-shell
 * @tags infra */

import type { Plugin } from 'vite'
import { readFileSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'

interface Options {
  /** Cache version suffix — bumped per deploy to force eviction (FR-13). */
  version: string
}

/**
 * Templates `public/sw.template.js` with the actual hashed asset filenames
 * emitted by the build, then writes `dist/sw.js`.
 *
 * Replacements are global (`/g`) — the placeholders may appear in both the
 * docstring header and the executable constants, and we want every site
 * substituted so the resulting comment is honest about the replacement
 * outcome.
 *
 *   __CACHE_NAME__  → `testudo-journal-${opts.version}`
 *   "[__SHELL__]"   → JSON array of shell paths to precache
 */
export function injectSwShell(opts: Options): Plugin {
  return {
    name: 'testudo-inject-sw-shell',
    apply: 'build',
    writeBundle(outOpts, bundle) {
      // Find the entry chunk (main JS) and the main CSS asset (if any).
      const entry = Object.values(bundle).find(
        (chunk): chunk is Extract<typeof chunk, { type: 'chunk' }> =>
          chunk.type === 'chunk' && chunk.isEntry === true,
      )
      const css = Object.values(bundle).find(
        (asset): asset is Extract<typeof asset, { type: 'asset' }> =>
          asset.type === 'asset' && /\.css$/.test(asset.fileName),
      )

      if (!entry) {
        // Fail loud — without an entry chunk the SW would precache nothing useful.
        throw new Error('[inject-sw-shell] no entry chunk found in bundle')
      }

      const shell: string[] = [
        '/',
        '/index.html',
        '/' + entry.fileName,
        ...(css ? ['/' + css.fileName] : []),
      ]

      const tmplPath = resolve(process.cwd(), 'public/sw.template.js')
      const tmpl = readFileSync(tmplPath, 'utf8')
      const out = tmpl
        .replace(/__CACHE_NAME__/g, `testudo-journal-${opts.version}`)
        .replace(/"\[__SHELL__\]"/g, JSON.stringify(shell))

      const outDir = outOpts.dir ?? resolve(process.cwd(), 'dist')
      writeFileSync(resolve(outDir, 'sw.js'), out)
    },
  }
}
