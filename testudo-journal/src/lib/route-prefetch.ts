import { prefetch, stableHash } from './cache'
import {
  fetchOverview,
  fetchEquityCurve,
  fetchTags,
  fetchLatestCoachReport,
  fetchDignitasMe,
} from '../api/client'

type Prefetcher = () => void

const EMPTY: Record<string, never> = {}

export const routePrefetchers: Record<string, Prefetcher> = {
  '/': () => {
    const h = stableHash(EMPTY)
    prefetch('overview:' + h, () => fetchOverview({}))
    prefetch('equity-curve:' + h, () => fetchEquityCurve({}))
  },
  '/trades': () => {
    prefetch('tags:all', () => fetchTags())
  },
  '/coach': () => {
    prefetch('overview:' + stableHash(EMPTY), () => fetchOverview({}))
    prefetch('coach-latest:', () => fetchLatestCoachReport())
  },
  '/dignitas': () => {
    prefetch('dignitas-me:', () => fetchDignitasMe())
  },
  '/journal': () => {
    prefetch('tags:all', () => fetchTags())
  },
}
