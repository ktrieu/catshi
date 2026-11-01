import { useState } from 'react'
import { scaleBand, scaleLinear } from 'd3-scale'
import { faDemocrat, faRepublican, type IconDefinition } from '@fortawesome/free-solid-svg-icons'
import { useElementWidth } from '../lib/useElementWidth'

export type Party = 'D' | 'R' | 'even'

export interface SenateMarginBucket {
  label: string
  party: Party
  probability: number
}

const COMPACT_BREAKPOINT = 640
const CORNER_RADIUS = 4
const MARGIN = { top: 28, bottom: 36 }

const WIDE_LAYOUT = { height: 380, sideWidth: 120, axisWidth: 44, iconWidth: 80 }
const COMPACT_LAYOUT = { height: 260, sideWidth: 0, axisWidth: 36, iconWidth: 0 }

const PARTY_FILL: Record<Party, string> = {
  D: 'fill-blue-600',
  R: 'fill-red-600',
  even: 'fill-gray-400',
}

const formatPercent = (p: number) => `${Math.round(p * 100)}%`

function topRoundedBar(x: number, y: number, width: number, height: number) {
  const r = Math.min(CORNER_RADIUS, width / 2, height)
  return [
    `M${x},${y + height}`,
    `V${y + r}`,
    `Q${x},${y} ${x + r},${y}`,
    `H${x + width - r}`,
    `Q${x + width},${y} ${x + width},${y + r}`,
    `V${y + height}`,
    'Z',
  ].join(' ')
}

export function SenateMarginChart({ data }: { data: SenateMarginBucket[] }) {
  const [containerRef, width] = useElementWidth<HTMLDivElement>()
  const compact = width < COMPACT_BREAKPOINT

  return (
    <div ref={containerRef} className="w-full">
      {width > 0 && compact && (
        <div className="mb-2 flex items-center justify-between">
          <PartyBadge icon={faDemocrat} name="DEMOCRATIC" fill="fill-blue-600" text="text-blue-600" />
          <PartyBadge icon={faRepublican} name="REPUBLICAN" fill="fill-red-600" text="text-red-600" iconRight />
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

  const { height, sideWidth, axisWidth, iconWidth } = compact ? COMPACT_LAYOUT : WIDE_LAYOUT
  const plotLeft = sideWidth + axisWidth
  const plotRight = width - sideWidth
  const plotBottom = height - MARGIN.bottom

  const x = scaleBand<string>()
    .domain(data.map((d) => d.label))
    .range([plotLeft, plotRight])
    .paddingInner(compact ? 0.1 : 0.15)
    .paddingOuter(0.1)

  const y = scaleLinear().domain([0, 1]).range([plotBottom, MARGIN.top])

  const centerY = (MARGIN.top + plotBottom) / 2

  return (
    <svg
      width={width}
      height={height}
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
            className="fill-gray-500 text-xs"
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
            <path
              d={topRoundedBar(bx, by, x.bandwidth(), plotBottom - by)}
              className={PARTY_FILL[d.party]}
              opacity={selected && !isSelected ? 0.6 : 1}
            />
            {isSelected && (
              <text
                x={bx + x.bandwidth() / 2}
                y={by - 6}
                textAnchor="middle"
                className="fill-gray-900 text-sm font-semibold"
              >
                {formatPercent(d.probability)}
              </text>
            )}
            {(showLabel || isSelected) && (
              <text
                x={bx + x.bandwidth() / 2}
                y={plotBottom + 20}
                textAnchor="middle"
                className={`text-xs ${isSelected ? 'fill-gray-900 font-semibold' : 'fill-gray-600'}`}
              >
                {d.label}
              </text>
            )}
          </g>
        )
      })}

      {!compact && (
        <>
          <PartyLabel icon={faDemocrat} name="DEMOCRATIC" cx={sideWidth / 2} cy={centerY} iconWidth={iconWidth} fill="fill-blue-600" />
          <PartyLabel icon={faRepublican} name="REPUBLICAN" cx={width - sideWidth / 2} cy={centerY} iconWidth={iconWidth} fill="fill-red-600" />
        </>
      )}
    </svg>
  )
}

function iconPathData(icon: IconDefinition) {
  const [iconWidth, iconHeight, , , pathData] = icon.icon
  return {
    iconWidth,
    iconHeight,
    d: Array.isArray(pathData) ? pathData.join(' ') : pathData,
  }
}

function PartyLabel({
  icon,
  name,
  cx,
  cy,
  iconWidth: targetWidth,
  fill,
}: {
  icon: IconDefinition
  name: string
  cx: number
  cy: number
  iconWidth: number
  fill: string
}) {
  const { iconWidth, iconHeight, d } = iconPathData(icon)
  const scale = targetWidth / iconWidth
  const height = iconHeight * scale

  return (
    <g>
      <path
        d={d}
        transform={`translate(${cx - targetWidth / 2}, ${cy - height}) scale(${scale})`}
        className={fill}
      />
      <text x={cx} y={cy + 22} textAnchor="middle" className={`${fill} text-sm font-bold tracking-wide`}>
        {name}
      </text>
    </g>
  )
}

function PartyBadge({
  icon,
  name,
  fill,
  text,
  iconRight = false,
}: {
  icon: IconDefinition
  name: string
  fill: string
  text: string
  iconRight?: boolean
}) {
  const { iconWidth, iconHeight, d } = iconPathData(icon)

  return (
    <div className={`flex items-center gap-2 ${iconRight ? 'flex-row-reverse' : ''}`}>
      <svg viewBox={`0 0 ${iconWidth} ${iconHeight}`} className="h-6 w-auto" aria-hidden>
        <path d={d} className={fill} />
      </svg>
      <span className={`${text} text-sm font-bold tracking-wide`}>{name}</span>
    </div>
  )
}
