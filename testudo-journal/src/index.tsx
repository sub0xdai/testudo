/* @refresh reload */
import { render } from 'solid-js/web'
import { Router, Route } from '@solidjs/router'
import { FilterProvider } from './components/filterContext'
import { Layout } from './components/Layout'
import { Overview } from './components/Overview'
import { Charts } from './components/Charts'
import { Trades } from './pages/Trades'
import { Journal } from './pages/Journal'
import './lib/echarts-setup'
import './lib/echarts-theme'
import './styles/app.css'

const root = document.getElementById('root')

render(
  () => (
    <FilterProvider>
      <Router root={Layout}>
        <Route path="/" component={Overview} />
        <Route path="/charts" component={Charts} />
        <Route path="/trades" component={Trades} />
        <Route path="/journal" component={Journal} />
      </Router>
    </FilterProvider>
  ),
  root!,
)
