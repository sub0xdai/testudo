/* @refresh reload */
import { render } from 'solid-js/web'
import { Router, Route } from '@solidjs/router'
import { FilterProvider } from './components/filterContext'
import { AuthProvider } from './context/AuthContext'
import { Layout } from './components/Layout'
import { Overview } from './components/Overview'
import { Trades } from './pages/Trades'
import { Journal } from './pages/Journal'
import './config/wallet' // initialize Reown AppKit
import './lib/echarts-setup'
import './lib/echarts-theme'
import './styles/app.css'

// Lazy-loaded pages
import { lazy } from 'solid-js'
const Account = lazy(() => import('./pages/Account'))
const Pair = lazy(() => import('./pages/Pair'))

const root = document.getElementById('root')

render(
  () => (
    <AuthProvider>
      <FilterProvider>
        <Router base="/desk" root={Layout}>
          <Route path="/pair" component={Pair} />
          <Route path="/" component={Overview} />
          <Route path="/trades" component={Trades} />
          <Route path="/journal" component={Journal} />
          <Route path="/account" component={Account} />
        </Router>
      </FilterProvider>
    </AuthProvider>
  ),
  root!,
)
