/** @anchor ui:journal:StorageBar
 * @tags ui */

import { createResource } from 'solid-js'
import { fetchStorageUsage } from '../../api/client'

function formatBytes(bytes: number): string {
  const MB = 1024 * 1024
  const KB = 1024
  if (bytes >= MB) return `${(bytes / MB).toFixed(1)}MB`
  if (bytes >= KB) return `${(bytes / KB).toFixed(1)}KB`
  return `${bytes}B`
}

export function StorageBar(props: { refreshKey?: number }) {
  const [usage] = createResource(() => props.refreshKey, fetchStorageUsage)

  const pct = () => {
    const u = usage()
    if (!u) return 0
    return Math.min(100, (u.used_bytes / u.quota_bytes) * 100)
  }

  const label = () => {
    const u = usage()
    if (!u) return ''
    return `${formatBytes(u.used_bytes)} / ${formatBytes(u.quota_bytes)}`
  }

  return (
    <div class="flex items-center gap-2" title={`${usage()?.image_count ?? 0} images stored`}>
      <div class="w-24 h-1.5 bg-container-bg border border-container-border overflow-hidden">
        <div
          class="h-full transition-all"
          classList={{
            'bg-signal-green/60': pct() < 70,
            'bg-signal-amber/60': pct() >= 70 && pct() < 90,
            'bg-signal-red/60': pct() >= 90,
          }}
          style={{ width: `${pct()}%` }}
        />
      </div>
      <span class="font-mono text-[10px] text-text-tertiary whitespace-nowrap">{label()}</span>
    </div>
  )
}
