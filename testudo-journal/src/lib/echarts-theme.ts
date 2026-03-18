import { echarts } from './echarts-setup'
import { TAG_PALETTE, CHART_BG } from './tokens'

export const TESTUDO_THEME = 'testudo-dark'

echarts.registerTheme(TESTUDO_THEME, {
  color: TAG_PALETTE,
  backgroundColor: 'transparent',
  textStyle: {
    fontFamily: "'Space Mono', monospace",
    color: '#555555',
    fontSize: 11,
  },
  title: {
    textStyle: { color: '#FFFFFF', fontFamily: "'Space Grotesk', sans-serif" },
  },
  legend: {
    textStyle: { color: '#888888', fontFamily: "'Space Mono', monospace", fontSize: 11 },
  },
  tooltip: {
    backgroundColor: CHART_BG,
    borderColor: '#3F3F46',
    borderWidth: 1,
    textStyle: {
      fontFamily: "'Space Mono', monospace",
      color: '#888888',
      fontSize: 11,
    },
    extraCssText: 'box-shadow: 0 4px 12px rgba(0,0,0,0.5);',
  },
  categoryAxis: {
    axisLine: { lineStyle: { color: '#3F3F46' } },
    axisTick: { lineStyle: { color: '#3F3F46' } },
    axisLabel: { color: '#555555' },
    splitLine: { lineStyle: { color: '#1A1A1A' } },
  },
  valueAxis: {
    axisLine: { lineStyle: { color: '#3F3F46' } },
    axisTick: { lineStyle: { color: '#3F3F46' } },
    axisLabel: { color: '#555555' },
    splitLine: { lineStyle: { color: '#1A1A1A' } },
  },
})
