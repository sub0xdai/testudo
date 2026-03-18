/* @refresh reload */
import { render } from 'solid-js/web'
import { Router, Route } from '@solidjs/router'
import { FilterProvider } from './components/filterContext'
import { Layout } from './components/Layout'
import { Overview } from './components/Overview'
import { Charts } from './components/Charts'
import './styles/app.css'

function TradesPlaceholder() {
  return <div class="font-mono text-text-secondary text-center py-20">TRADES — COMING SOON</div>
}

function JournalPlaceholder() {
  return <div class="font-mono text-text-secondary text-center py-20">JOURNAL — COMING SOON</div>
}

const root = document.getElementById('root')

render(
  () => (
    <FilterProvider>
      <Router root={Layout}>
        <Route path="/" component={Overview} />
        <Route path="/charts" component={Charts} />
        <Route path="/trades" component={TradesPlaceholder} />
        <Route path="/journal" component={JournalPlaceholder} />
      </Router>
    </FilterProvider>
  ),
  root!,
)
