import { useState } from 'react'
import { scaleBand, scaleLinear } from 'd3-scale'
import { faDemocrat, faRepublican, type IconDefinition } from '@fortawesome/free-solid-svg-icons'

export type Party = 'D' | 'R' | 'even'

export interface SenateMarginBucket {
  label: string
  party: Party
  probability: number
}

const WIDTH = 960
const HEIGHT = 380
const SIDE_WIDTH = 120
const AXIS_WIDTH = 44
const MARGIN = { top: 28, bottom: 36 }
const PLOT_LEFT = SIDE_WIDTH + AXIS_WIDTH
const PLOT_RIGHT = WIDTH - SIDE_WIDTH
const PLOT_BOTTOM = HEIGHT - MARGIN.bottom
const ICON_WIDTH = 80
const CORNER_RADIUS = 4

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
  const [hovered, setHovered] = useState<string | null>(null)

  const x = scaleBand<string>()
    .domain(data.map((d) => d.label))
    .range([PLOT_LEFT, PLOT_RIGHT])
    .paddingInner(0.15)
    .paddingOuter(0.1)

  const y = scaleLinear().domain([0, 1]).range([PLOT_BOTTOM, MARGIN.top])

  const centerY = (MARGIN.top + PLOT_BOTTOM) / 2

  return (
    <svg
      viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
      className="h-auto w-full"
      role="img"
      aria-label="Probability of each Senate seat differential"
    >
      {y.ticks(5).map((tick) => (
        <g key={tick}>
          <line
            x1={PLOT_LEFT}
            x2={PLOT_RIGHT}
            y1={y(tick)}
            y2={y(tick)}
            className={tick === 0 ? 'stroke-gray-400' : 'stroke-gray-200'}
          />
          <text
            x={PLOT_LEFT - 8}
            y={y(tick)}
            textAnchor="end"
            dominantBaseline="middle"
            className="fill-gray-500 text-xs"
          >
            {formatPercent(tick)}
          </text>
        </g>
      ))}

      {data.map((d) => {
        const bx = x(d.label) ?? 0
        const by = y(d.probability)
        const isHovered = hovered === d.label
        return (
          <g
            key={d.label}
            onMouseEnter={() => setHovered(d.label)}
            onMouseLeave={() => setHovered(null)}
          >
            <rect
              x={bx}
              y={MARGIN.top}
              width={x.bandwidth()}
              height={PLOT_BOTTOM - MARGIN.top}
              className={isHovered ? 'fill-gray-100' : 'fill-transparent'}
            />
            <path
              d={topRoundedBar(bx, by, x.bandwidth(), PLOT_BOTTOM - by)}
              className={PARTY_FILL[d.party]}
              opacity={hovered && !isHovered ? 0.6 : 1}
            />
            {isHovered && (
              <text
                x={bx + x.bandwidth() / 2}
                y={by - 6}
                textAnchor="middle"
                className="fill-gray-900 text-sm font-semibold"
              >
                {formatPercent(d.probability)}
              </text>
            )}
            <text
              x={bx + x.bandwidth() / 2}
              y={PLOT_BOTTOM + 20}
              textAnchor="middle"
              className="fill-gray-600 text-xs"
            >
              {d.label}
            </text>
          </g>
        )
      })}

      <PartyLabel icon={faDemocrat} name="DEMOCRATIC" cx={SIDE_WIDTH / 2} cy={centerY} fill="fill-blue-600" />
      <PartyLabel icon={faRepublican} name="REPUBLICAN" cx={WIDTH - SIDE_WIDTH / 2} cy={centerY} fill="fill-red-600" />
    </svg>
  )
}

function PartyLabel({
  icon,
  name,
  cx,
  cy,
  fill,
}: {
  icon: IconDefinition
  name: string
  cx: number
  cy: number
  fill: string
}) {
  const [iconWidth, iconHeight, , , pathData] = icon.icon
  const scale = ICON_WIDTH / iconWidth
  const height = iconHeight * scale

  return (
    <g>
      <path
        d={Array.isArray(pathData) ? pathData.join(' ') : pathData}
        transform={`translate(${cx - ICON_WIDTH / 2}, ${cy - height}) scale(${scale})`}
        className={fill}
      />
      <text x={cx} y={cy + 22} textAnchor="middle" className={`${fill} text-sm font-bold tracking-wide`}>
        {name}
      </text>
    </g>
  )
}
