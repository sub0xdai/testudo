#!/usr/bin/env node
// Budget: main entry chunk must be ≤ 250 KB gzipped (PERF-01 FR-3)
import { readFileSync, readdirSync } from 'fs'
import { join, resolve } from 'path'
import { gzipSync } from 'zlib'

const MAIN_ENTRY_GZ_BUDGET = 250 * 1024 // 250 KB
const DIST_ASSETS = resolve(import.meta.dirname, '../dist/assets')

// Find main entry chunks: files starting with "index-" and ending in ".js"
const files = readdirSync(DIST_ASSETS)
const entryChunks = files.filter(f => f.startsWith('index-') && f.endsWith('.js'))

if (entryChunks.length === 0) {
  console.error('[budget] No main entry chunk found in dist/assets. Run vite build first.')
  process.exit(1)
}

let failed = false
const rows = []

for (const chunk of entryChunks) {
  const raw = readFileSync(join(DIST_ASSETS, chunk))
  const gz = gzipSync(raw)
  const overBy = gz.length - MAIN_ENTRY_GZ_BUDGET
  const status = overBy > 0 ? '❌ OVER' : '✅ OK'
  rows.push({ chunk, raw: raw.length, gz: gz.length, overBy, status })
  if (overBy > 0) failed = true
}

console.log('\n[budget] Main entry chunk gzip sizes:\n')
console.log(`${'Chunk'.padEnd(40)} ${'Raw'.padStart(10)} ${'Gzipped'.padStart(10)} ${'Budget'.padStart(10)} Status`)
console.log('-'.repeat(80))
for (const r of rows) {
  const over = r.overBy > 0 ? `  (+${(r.overBy / 1024).toFixed(1)} KB over)` : ''
  console.log(
    `${r.chunk.padEnd(40)} ${(r.raw / 1024).toFixed(1).padStart(9)}K ${(r.gz / 1024).toFixed(1).padStart(9)}K ${(MAIN_ENTRY_GZ_BUDGET / 1024).toFixed(0).padStart(9)}K ${r.status}${over}`
  )
}
console.log()

if (failed) {
  console.error('[budget] FAIL — main entry chunk exceeds 250 KB gzip budget.')
  process.exit(1)
} else {
  console.log('[budget] PASS — all entry chunks within budget.')
}
