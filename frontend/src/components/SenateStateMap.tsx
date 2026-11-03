import { useMemo, useState } from 'react'
import { geoPath } from 'd3-geo'
import { scaleLinear } from 'd3-scale'
import { feature, mesh } from 'topojson-client'
import type { GeometryCollection, Topology } from 'topojson-specification'
import type { Feature, Geometry } from 'geojson'
import statesTopology from 'us-atlas/states-albers-10m.json'

export interface SwingStateOdds {
  fips: string
  name: string
  tag: string
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

export function SenateStateMap({
  swingStates,
  selectedFips,
  onSelect,
}: {
  swingStates: SwingStateOdds[]
  selectedFips: string | null
  onSelect: (odds: SwingStateOdds) => void
}) {
  const oddsByFips = useMemo(() => new Map(swingStates.map((s) => [s.fips, s])), [swingStates])

  const [activeFips, setActiveFips] = useState<string | null>(null)

  const handleSelect = onSelect
  const selectedState = selectedFips ? states.find((s) => String(s.id) === selectedFips) : undefined
  const activeState =
    activeFips && activeFips !== selectedFips ? states.find((s) => String(s.id) === activeFips) : undefined

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
              className="cursor-pointer stroke-white outline-none"
              strokeWidth={1.5}
              onPointerEnter={(e) => e.pointerType === 'mouse' && setActiveFips(odds.fips)}
              onPointerLeave={(e) => e.pointerType === 'mouse' && setActiveFips(null)}
              onFocus={(e) => e.currentTarget.matches(':focus-visible') && setActiveFips(odds.fips)}
              onBlur={() => setActiveFips(null)}
              strokeLinejoin="round"
              role="button"
              tabIndex={0}
              aria-label={label}
              aria-pressed={selectedFips === odds.fips}
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

      {activeState && (
        <path
          d={path(activeState) ?? ''}
          fill="none"
          className="pointer-events-none stroke-gray-900"
          strokeWidth={1.5}
          strokeLinejoin="round"
        />
      )}

      {selectedState && (
        <path
          d={path(selectedState) ?? ''}
          fill="none"
          className="pointer-events-none stroke-gray-900"
          strokeWidth={3}
          strokeLinejoin="round"
        />
      )}
    </svg>
  )
}
