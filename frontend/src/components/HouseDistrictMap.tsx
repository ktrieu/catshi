import { useState } from 'react'
import { scaleLinear } from 'd3-scale'
import njSevenSvg from '../assets/nj-7.svg?raw'

const oddsColor = scaleLinear<string>()
  .domain([0.2, 0.5, 0.8])
  .range(['#dc2626', '#a78bfa', '#2563eb'])
  .clamp(true)

const formatPercent = (p: number) => `${Math.round(p * 100)}%`

const viewBox = /viewBox="([^"]+)"/.exec(njSevenSvg)?.[1] ?? '0 0 100 100'
const pathData = /<path[^>]*\sd="([^"]+)"/.exec(njSevenSvg)?.[1] ?? ''

export function HouseDistrictMap({
  title,
  demProbability,
  selected,
  onSelect,
}: {
  title: string
  demProbability: number
  selected: boolean
  onSelect: () => void
}) {
  const [active, setActive] = useState(false)
  const label = `${title}: Democratic ${formatPercent(demProbability)}, Republican ${formatPercent(1 - demProbability)}`
  return (
    <figure className="w-32 shrink-0">
      <svg
        viewBox={viewBox}
        className="block h-auto w-full overflow-visible"
        role="group"
        aria-label={`${title} district map`}
      >
        <path
          d={pathData}
          fill={oddsColor(demProbability)}
          className="cursor-pointer stroke-gray-900 outline-none"
          strokeWidth={selected ? 3 : active ? 1.5 : 0}
          vectorEffect="non-scaling-stroke"
          strokeLinejoin="round"
          role="button"
          tabIndex={0}
          aria-label={label}
          aria-pressed={selected}
          onPointerEnter={(e) => e.pointerType === 'mouse' && setActive(true)}
          onPointerLeave={(e) => e.pointerType === 'mouse' && setActive(false)}
          onFocus={(e) => e.currentTarget.matches(':focus-visible') && setActive(true)}
          onBlur={() => setActive(false)}
          onClick={onSelect}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault()
              onSelect()
            }
          }}
        >
          <title>{label}</title>
        </path>
      </svg>
      <figcaption className="mt-2 text-center text-base font-bold">{title}</figcaption>
    </figure>
  )
}
