/** @anchor ui:journal:EChart
 * @tags ui */

import { onMount, onCleanup, createEffect, type Accessor } from 'solid-js'
import { echarts } from '../../lib/echarts-setup'
import { TESTUDO_THEME, registerTestudoTheme } from '../../lib/echarts-theme'
import { onThemeChange } from '../../lib/theme-observer'
import type { EChartsOption } from 'echarts'

interface EChartProps {
  option: Accessor<EChartsOption | undefined>
  class?: string
  height?: string
}

export function EChart(props: EChartProps) {
  let container!: HTMLDivElement
  let chart: ReturnType<typeof echarts.init> | undefined
  let lastOption: EChartsOption | undefined

  function initChart() {
    chart?.dispose()
    registerTestudoTheme()
    chart = echarts.init(container, TESTUDO_THEME)
    if (lastOption) {
      chart.setOption(lastOption, { notMerge: true })
    }
  }

  onMount(() => {
    initChart()

    const resizeObserver = new ResizeObserver(() => chart?.resize())
    resizeObserver.observe(container)

    const unsubTheme = onThemeChange(() => {
      initChart()
    })

    onCleanup(() => {
      resizeObserver.disconnect()
      unsubTheme()
      chart?.dispose()
    })
  })

  createEffect(() => {
    const opt = props.option()
    if (opt && chart) {
      lastOption = opt
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
