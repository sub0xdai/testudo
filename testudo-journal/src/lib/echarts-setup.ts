/** @anchor infra:journal-lib:echarts-setup
 * @tags infra */

import * as echarts from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { PieChart, BarChart, ScatterChart, HeatmapChart, TreemapChart, LineChart, RadarChart } from 'echarts/charts'
import {
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  VisualMapComponent,
  CalendarComponent,
  MarkLineComponent,
  RadarComponent,
} from 'echarts/components'

echarts.use([
  CanvasRenderer,
  PieChart,
  BarChart,
  ScatterChart,
  HeatmapChart,
  TreemapChart,
  LineChart,
  RadarChart,
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  VisualMapComponent,
  CalendarComponent,
  MarkLineComponent,
  RadarComponent,
])

export { echarts }
