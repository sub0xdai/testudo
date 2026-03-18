import { onMount, onCleanup, createEffect, type Accessor } from 'solid-js'
import { echarts } from '../../lib/echarts-setup'
import { TESTUDO_THEME } from '../../lib/echarts-theme'
import type { EChartsOption } from 'echarts'

interface EChartProps {
  option: Accessor<EChartsOption | undefined>
  class?: string
  height?: string
}

export function EChart(props: EChartProps) {
  let container!: HTMLDivElement
  let chart: ReturnType<typeof echarts.init> | undefined

  onMount(() => {
    chart = echarts.init(container, TESTUDO_THEME)

    const observer = new ResizeObserver(() => chart?.resize())
    observer.observe(container)

    onCleanup(() => {
      observer.disconnect()
      chart?.dispose()
    })
  })

  createEffect(() => {
    const opt = props.option()
    if (opt && chart) {
      chart.setOption(opt, { notMerge: true })
    }
  })

  return (
    <div
      ref={container!}
      class={props.class}
      style={{ height: props.height ?? '224px', width: '100%' }}
    />
  )
}
