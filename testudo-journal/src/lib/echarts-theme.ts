/** @anchor infra:journal-lib:echarts-theme
 * @tags infra */

import { echarts } from './echarts-setup'
import { getTagPalette, getChartBg, getBorder, getTextTertiary, getTextSecondary, getTextPrimary, getBgHover } from './tokens'

export const TESTUDO_THEME = 'testudo-dark'

/** Register (or re-register) the Testudo ECharts theme with current CSS var values */
export function registerTestudoTheme() {
  echarts.registerTheme(TESTUDO_THEME, {
    color: getTagPalette(),
    backgroundColor: 'transparent',
    textStyle: {
      fontFamily: "'Space Mono', monospace",
      color: getTextTertiary(),
      fontSize: 11,
    },
    title: {
      textStyle: { color: getTextPrimary(), fontFamily: "'Space Grotesk', sans-serif" },
    },
    legend: {
      textStyle: { color: getTextSecondary(), fontFamily: "'Space Mono', monospace", fontSize: 11 },
    },
    tooltip: {
      backgroundColor: getChartBg(),
      borderColor: getBorder(),
      borderWidth: 1,
      textStyle: {
        fontFamily: "'Space Mono', monospace",
        color: getTextSecondary(),
        fontSize: 11,
      },
      extraCssText: 'box-shadow: 0 4px 12px rgba(0,0,0,0.5);',
    },
    categoryAxis: {
      axisLine: { lineStyle: { color: getBorder() } },
      axisTick: { lineStyle: { color: getBorder() } },
      axisLabel: { color: getTextTertiary() },
      splitLine: { lineStyle: { color: getBgHover() } },
    },
    valueAxis: {
      axisLine: { lineStyle: { color: getBorder() } },
      axisTick: { lineStyle: { color: getBorder() } },
      axisLabel: { color: getTextTertiary() },
      splitLine: { lineStyle: { color: getBgHover() } },
    },
  })
}

// Register on initial load
registerTestudoTheme()
