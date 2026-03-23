import * as echarts from 'echarts/core'
import { CanvasRenderer } from 'echarts/renderers'
import { PieChart, BarChart, ScatterChart, HeatmapChart, TreemapChart, LineChart } from 'echarts/charts'
import {
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  VisualMapComponent,
  CalendarComponent,
  MarkLineComponent,
} from 'echarts/components'

echarts.use([
  CanvasRenderer,
  PieChart,
  BarChart,
  ScatterChart,
  HeatmapChart,
  TreemapChart,
  LineChart,
  TitleComponent,
  TooltipComponent,
  LegendComponent,
  GridComponent,
  VisualMapComponent,
  CalendarComponent,
  MarkLineComponent,
])

export { echarts }
