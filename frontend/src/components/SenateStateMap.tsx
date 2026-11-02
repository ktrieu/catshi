import { useMemo } from 'react'
import { geoPath } from 'd3-geo'
import { scaleLinear } from 'd3-scale'
import { feature, mesh } from 'topojson-client'
import type { GeometryCollection, Topology } from 'topojson-specification'
import type { Feature, Geometry } from 'geojson'
import statesTopology from 'us-atlas/states-albers-10m.json'

export interface SwingStateOdds {
  fips: string
  name: string
  demProbability: number
}

type StateFeature = Feature<Geometry, { name: string }>

const MAP_WIDTH = 975
const MAP_HEIGHT = 610

const oddsColor = scaleLinear<string>()
  .domain([0.2, 0.5, 0.8])
  .range(['#dc2626', '#a78bfa', '#2563eb'])
  .clamp(true)

const formatPercent = (p: number) => `${Math.round(p * 100)}%`

const topology = statesTopology as unknown as Topology<{ states: GeometryCollection<{ name: string }> }>
const path = geoPath()
const states = (feature(topology, topology.objects.states) as { features: StateFeature[] }).features
const borders = path(mesh(topology, topology.objects.states, (a, b) => a !== b)) ?? ''

export function SenateStateMap({ swingStates }: { swingStates: SwingStateOdds[] }) {
  const oddsByFips = useMemo(() => new Map(swingStates.map((s) => [s.fips, s])), [swingStates])

  const handleSelect = (odds: SwingStateOdds) => {
    console.log('Swing state clicked:', odds)
  }

  return (
    <svg
      viewBox={`0 0 ${MAP_WIDTH} ${MAP_HEIGHT}`}
      className="block h-auto w-full"
      role="group"
      aria-label="Senate swing states map"
    >
      <g className="fill-gray-200">
        {states
          .filter((s) => !oddsByFips.has(String(s.id)))
          .map((s) => (
            <path key={s.id} d={path(s) ?? ''} />
          ))}
      </g>
      <path d={borders} fill="none" className="stroke-white" strokeWidth={1} strokeLinejoin="round" />

      {states
        .filter((s) => oddsByFips.has(String(s.id)))
        .map((s) => {
          const odds = oddsByFips.get(String(s.id))!
          const label = `${odds.name}: Democratic ${formatPercent(odds.demProbability)}, Republican ${formatPercent(1 - odds.demProbability)}`
          return (
            <path
              key={s.id}
              d={path(s) ?? ''}
              fill={oddsColor(odds.demProbability)}
              className="cursor-pointer stroke-white outline-none hover:stroke-gray-900 focus-visible:stroke-gray-900"
              strokeWidth={1.5}
              strokeLinejoin="round"
              role="button"
              tabIndex={0}
              aria-label={label}
              onClick={() => handleSelect(odds)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault()
                  handleSelect(odds)
                }
              }}
            >
              <title>{label}</title>
            </path>
          )
        })}
    </svg>
  )
}
