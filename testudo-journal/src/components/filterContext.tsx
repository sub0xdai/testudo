import { createContext, useContext, type JSX } from 'solid-js'
import { createSignal } from 'solid-js'
import type { StatsFilter } from '../api/client'

type FilterContextType = {
  filters: () => StatsFilter
  setFilters: (f: StatsFilter) => void
}

const FilterContext = createContext<FilterContextType>()

export function FilterProvider(props: { children: JSX.Element }) {
  const [filters, setFilters] = createSignal<StatsFilter>({})

  return (
    <FilterContext.Provider value={{ filters, setFilters }}>
      {props.children}
    </FilterContext.Provider>
  )
}

export function useFilters(): FilterContextType {
  const ctx = useContext(FilterContext)
  if (!ctx) throw new Error('useFilters must be used within FilterProvider')
  return ctx
}
