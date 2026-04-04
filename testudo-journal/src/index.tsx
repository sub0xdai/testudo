/* @refresh reload */
import { render } from 'solid-js/web'
import { Router, Route, Navigate } from '@solidjs/router'
import { FilterProvider } from './components/filterContext'
import { AuthProvider } from './context/AuthContext'
import { Layout } from './components/Layout'
import './config/wallet' // initialize Reown AppKit
import './styles/app.css'

// Lazy-loaded pages — keep main bundle lean
import { lazy } from 'solid-js'
const Overview = lazy(() => import('./components/Overview').then(m => ({ default: m.Overview })))
const Trades = lazy(() => import('./pages/Trades').then(m => ({ default: m.Trades })))
const Journal = lazy(() => import('./pages/Journal').then(m => ({ default: m.Journal })))
const Account = lazy(() => import('./pages/Account'))
const Pair = lazy(() => import('./pages/Pair'))

// Redirect bare root to /desk/ — router base doesn't catch paths outside /desk/*
if (window.location.pathname === '/' || window.location.pathname === '') {
  window.location.replace('/desk/')
}

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
          <Route path="/*" component={() => <Navigate href="/" />} />
        </Router>
      </FilterProvider>
    </AuthProvider>
  ),
  root!,
)
