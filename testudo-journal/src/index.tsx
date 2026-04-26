/* @refresh reload */
import { render } from 'solid-js/web'
import { Router, Route, Navigate } from '@solidjs/router'
import { FilterProvider } from './components/filterContext'
import { AuthProvider } from './context/AuthContext'
import { Layout } from './components/Layout'
import './styles/app.css'

// Lazy-loaded pages — keep main bundle lean
import { lazy } from 'solid-js'
const Overview = lazy(() => import('./components/Overview').then(m => ({ default: m.Overview })))
const Trades = lazy(() => import('./pages/Trades').then(m => ({ default: m.Trades })))
const Journal = lazy(() => import('./pages/Journal').then(m => ({ default: m.Journal })))
const Account = lazy(() => import('./pages/Account'))
const Pair = lazy(() => import('./pages/Pair'))
const Coach = lazy(() => import('./pages/Coach'))
const Dignitas = lazy(() => import('./pages/Dignitas'))
const PublicProfile = lazy(() => import('./pages/PublicProfile'))

// Redirect bare root to /desk/ — router base doesn't catch paths outside /desk/*
if (window.location.pathname === '/' || window.location.pathname === '') {
  window.location.replace('/desk/')
}

function preconnect(rawUrl: string): void {
  if (!rawUrl) return
  try {
    const u = new URL(rawUrl)
    if (u.protocol === 'ws:') u.protocol = 'http:'
    else if (u.protocol === 'wss:') u.protocol = 'https:'
    const link = document.createElement('link')
    link.rel = 'preconnect'
    link.href = u.origin
    link.crossOrigin = 'anonymous'
    document.head.appendChild(link)
  } catch { /* malformed URL — skip */ }
}
preconnect(import.meta.env.VITE_API_URL ?? '')
preconnect(import.meta.env.VITE_WS_URL ?? '')

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
          <Route path="/coach" component={Coach} />
          <Route path="/dignitas" component={Dignitas} />
          <Route path="/d/:handle" component={PublicProfile} />
          <Route path="/*" component={() => <Navigate href="/" />} />
        </Router>
      </FilterProvider>
    </AuthProvider>
  ),
  root!,
)
