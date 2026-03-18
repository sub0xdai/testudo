import { createResource, For } from 'solid-js'
import { ChartContainer } from './ChartContainer'
import { useFilters } from '../filterContext'
import { fetchTimeDistribution } from '../../api/client'
import { signalGreenAlpha } from '../../lib/tokens'

const DAYS = ['SUN', 'MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT']
const HOURS = Array.from({ length: 24 }, (_, i) => i)

function intensityColor(count: number, maxCount: number): string {
  if (count === 0 || maxCount === 0) return '#1A1A1A'
  const ratio = count / maxCount
  if (ratio < 0.25) return signalGreenAlpha(0.15)
  if (ratio < 0.5) return signalGreenAlpha(0.3)
  if (ratio < 0.75) return signalGreenAlpha(0.55)
  return signalGreenAlpha(0.8)
}

export function TimeHeatmap() {
  const { filters } = useFilters()
  const [data] = createResource(filters, fetchTimeDistribution)

  function grid() {
    const d = data()
    if (!d?.data?.length) return { cells: [], maxCount: 0 }

    const map = new Map<string, number>()
    let maxCount = 0
    for (const slot of d.data) {
      const key = `${slot.day_of_week}-${slot.hour}`
      map.set(key, slot.trade_count)
      if (slot.trade_count > maxCount) maxCount = slot.trade_count
    }

    const cells: { day: number; hour: number; count: number }[] = []
    for (const day of [0, 1, 2, 3, 4, 5, 6]) {
      for (const hour of HOURS) {
        cells.push({ day, hour, count: map.get(`${day}-${hour}`) ?? 0 })
      }
    }
    return { cells, maxCount }
  }

  return (
    <ChartContainer title="TIME DISTRIBUTION" loading={data.loading} empty={!data()?.data?.length}>
      <div class="overflow-x-auto">
        <div class="min-w-[500px]">
          {/* Hour labels */}
          <div class="flex ml-10 mb-1">
            <For each={HOURS.filter((h) => h % 3 === 0)}>
              {(h) => (
                <div class="font-mono text-[9px] text-text-tertiary" style={{ width: `${(3 / 24) * 100}%` }}>
                  {String(h).padStart(2, '0')}
                </div>
              )}
            </For>
          </div>

          {/* Grid rows */}
          <For each={[0, 1, 2, 3, 4, 5, 6]}>
            {(day) => (
              <div class="flex items-center gap-1 mb-0.5">
                <span class="font-mono text-[10px] text-text-tertiary w-9 text-right">{DAYS[day]}</span>
                <div class="flex flex-1 gap-px">
                  <For each={HOURS}>
                    {(hour) => {
                      const g = grid()
                      const cell = g.cells.find((c) => c.day === day && c.hour === hour)
                      return (
                        <div
                          class="flex-1 h-4 rounded-sm"
                          style={{ "background-color": intensityColor(cell?.count ?? 0, g.maxCount) }}
                          title={`${DAYS[day]} ${String(hour).padStart(2, '0')}:00 — ${cell?.count ?? 0} trades`}
                        />
                      )
                    }}
                  </For>
                </div>
              </div>
            )}
          </For>
        </div>
      </div>
    </ChartContainer>
  )
}
