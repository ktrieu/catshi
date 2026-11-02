import { useState } from 'react'
import { scaleBand, scaleLinear } from 'd3-scale'
import { useElementWidth } from '../lib/useElementWidth'

export type Party = 'D' | 'R' | 'even'

export interface SenateMarginBucket {
  label: string
  party: Party
  probability: number
}

const COMPACT_BREAKPOINT = 640
const MARGIN = { top: 28, bottom: 36 }

const HEIGHT = 240
const WIDE_AXIS_WIDTH = 44
const COMPACT_AXIS_WIDTH = 36

const PARTY_FILL: Record<Party, string> = {
  D: 'fill-blue-600',
  R: 'fill-red-600',
  even: 'fill-gray-400',
}

const formatPercent = (p: number) => `${Math.round(p * 100)}%`

export function SenateMarginChart({ data }: { data: SenateMarginBucket[] }) {
  const [containerRef, width] = useElementWidth<HTMLDivElement>()
  const compact = width < COMPACT_BREAKPOINT

  return (
    <div ref={containerRef} className="w-full">
      {width > 0 && (
        <div className="mb-2 flex items-center justify-between">
          <PartyLabel name="DEMOCRATIC" className="text-blue-600" compact={compact} />
          <PartyLabel name="REPUBLICAN" className="text-red-600" compact={compact} />
        </div>
      )}
      {width > 0 && <Chart data={data} width={width} compact={compact} />}
    </div>
  )
}

function Chart({
  data,
  width,
  compact,
}: {
  data: SenateMarginBucket[]
  width: number
  compact: boolean
}) {
  const [selected, setSelected] = useState<string | null>(null)

  const plotLeft = compact ? COMPACT_AXIS_WIDTH : WIDE_AXIS_WIDTH
  const plotRight = width
  const plotBottom = HEIGHT - MARGIN.bottom

  const x = scaleBand<string>()
    .domain(data.map((d) => d.label))
    .range([plotLeft, plotRight])
    .paddingInner(compact ? 0.1 : 0.15)
    .paddingOuter(0.1)

  const y = scaleLinear().domain([0, 1]).range([plotBottom, MARGIN.top])

  return (
    <svg
      width={width}
      height={HEIGHT}
      className="block"
      role="img"
      aria-label="Probability of each Senate seat differential"
      onClick={() => setSelected(null)}
    >
      {y.ticks(5).map((tick) => (
        <g key={tick}>
          <line
            x1={plotLeft}
            x2={plotRight}
            y1={y(tick)}
            y2={y(tick)}
            className={tick === 0 ? 'stroke-gray-400' : 'stroke-gray-200'}
          />
          <text
            x={plotLeft - 8}
            y={y(tick)}
            textAnchor="end"
            dominantBaseline="middle"
            className={`fill-gray-500 ${compact ? 'text-xs' : 'text-sm'}`}
          >
            {formatPercent(tick)}
          </text>
        </g>
      ))}

      {data.map((d, i) => {
        const bx = x(d.label) ?? 0
        const by = y(d.probability)
        const isSelected = selected === d.label
        const showLabel = !compact || i % 2 === 0
        return (
          <g
            key={d.label}
            onPointerEnter={(e) => e.pointerType === 'mouse' && setSelected(d.label)}
            onPointerLeave={(e) => e.pointerType === 'mouse' && setSelected(null)}
            onClick={(e) => {
              e.stopPropagation()
              setSelected(d.label)
            }}
          >
            <rect
              x={bx}
              y={MARGIN.top}
              width={x.bandwidth()}
              height={plotBottom - MARGIN.top}
              className={isSelected ? 'fill-gray-100' : 'fill-transparent'}
            />
            <rect
              x={bx - (x.step() - x.bandwidth()) / 2}
              y={MARGIN.top}
              width={x.step()}
              height={plotBottom - MARGIN.top}
              className="fill-transparent"
            />
            <rect
              x={bx}
              y={by}
              width={x.bandwidth()}
              height={plotBottom - by}
              className={PARTY_FILL[d.party]}
              opacity={selected && !isSelected ? 0.6 : 1}
            />
            {isSelected && (
              <text
                x={bx + x.bandwidth() / 2}
                y={by - 6}
                textAnchor="middle"
                className={`fill-gray-900 font-semibold ${compact ? 'text-sm' : 'text-base'}`}
              >
                {formatPercent(d.probability)}
              </text>
            )}
            {(showLabel || isSelected) && (
              <text
                x={bx + x.bandwidth() / 2}
                y={plotBottom + 20}
                textAnchor="middle"
                className={`${compact ? 'text-xs' : 'text-sm'} ${isSelected ? 'fill-gray-900 font-semibold' : 'fill-gray-600'}`}
              >
                {d.label}
              </text>
            )}
          </g>
        )
      })}
    </svg>
  )
}

function PartyLabel({ name, className, compact }: { name: string; className: string; compact: boolean }) {
  return <span className={`${className} ${compact ? 'text-sm' : 'text-lg'} font-bold tracking-wide`}>{name}</span>
}
