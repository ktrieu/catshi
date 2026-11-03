import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'
import { SenateMarginChart, type Party, type SenateMarginBucket } from '../../components/SenateMarginChart'
import { SenateStateMap, type SwingStateOdds } from '../../components/SenateStateMap'
import { TradeUi } from '../../components/TradeUi'
import { elxn26MarketsQueryOptions, type ApiMarket, type Elxn26Markets } from '../../lib/api'

export const Route = createFileRoute('/_app/election-2026')({
  component: Election2026,
})

const PRICE_SCALE = 1000
const TOPLINE_TAG = 'elxn26-senate-margin'

const SWING_STATES: { tag: string; fips: string; name: string }[] = [
  { tag: 'elxn26-senate-nc', fips: '37', name: 'North Carolina' },
  { tag: 'elxn26-senate-me', fips: '23', name: 'Maine' },
  { tag: 'elxn26-senate-mi', fips: '26', name: 'Michigan' },
  { tag: 'elxn26-senate-oh', fips: '39', name: 'Ohio' },
  { tag: 'elxn26-senate-ia', fips: '19', name: 'Iowa' },
  { tag: 'elxn26-senate-tx', fips: '48', name: 'Texas' },
  { tag: 'elxn26-senate-ak', fips: '02', name: 'Alaska' },
  { tag: 'elxn26-senate-sc', fips: '45', name: 'South Carolina' },
  { tag: 'elxn26-senate-ne', fips: '31', name: 'Nebraska' },
  { tag: 'elxn26-senate-ks', fips: '20', name: 'Kansas' },
  { tag: 'elxn26-senate-ga', fips: '13', name: 'Georgia' },
  { tag: 'elxn26-senate-nh', fips: '33', name: 'New Hampshire' },
]

const MARGIN_BUCKETS: { instrument: string; label: string; party: Party }[] = [
  { instrument: 'D+12+', label: 'D+12+', party: 'D' },
  { instrument: 'D+10', label: 'D+10', party: 'D' },
  { instrument: 'D+8', label: 'D+8', party: 'D' },
  { instrument: 'D+6', label: 'D+6', party: 'D' },
  { instrument: 'D+4', label: 'D+4', party: 'D' },
  { instrument: 'D+2', label: 'D+2', party: 'D' },
  { instrument: 'Even', label: 'Even', party: 'even' },
  { instrument: 'R+2+', label: 'R+2+', party: 'R' },
]

function toMarginBuckets(market: ApiMarket | undefined): SenateMarginBucket[] {
  if (!market) return []
  const priceByName = new Map(market.instruments.map((i) => [i.name, i.price]))
  return MARGIN_BUCKETS.map(({ instrument, label, party }) => ({
    label,
    party,
    probability: (priceByName.get(instrument) ?? 0) / PRICE_SCALE,
  }))
}

function toSwingStates(markets: Elxn26Markets): SwingStateOdds[] {
  return SWING_STATES.flatMap(({ tag, fips, name }) => {
    const market = markets[tag]
    const blue = market?.instruments.find((i) => i.name.endsWith('(D)') || i.name.endsWith('(I)'))
    if (!blue) return []
    return [{ fips, name, tag, demProbability: blue.price / PRICE_SCALE }]
  })
}

function Election2026() {
  const { data, isPending, isError } = useQuery(elxn26MarketsQueryOptions)
  const [selectedTag, setSelectedTag] = useState(TOPLINE_TAG)
  const selectedMarket = data?.[selectedTag] ?? data?.[TOPLINE_TAG]
  const selectedFips = SWING_STATES.find((s) => s.tag === selectedTag)?.fips ?? null

  return (
    <section className="py-6">
      <h1 className="text-3xl font-bold mb-6">ELECTION 2026</h1>
      <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
        <div className="min-w-0 md:col-span-2">
          <h3 className="text-2xl font-bold mb-6">SENATE</h3>
          {isPending && <p className="text-gray-500">Loading markets…</p>}
          {isError && <p className="text-red-600">Failed to load markets.</p>}
          {data && (
            <>
              <SenateMarginChart
                data={toMarginBuckets(data[TOPLINE_TAG])}
                onSelect={() => setSelectedTag(TOPLINE_TAG)}
              />
              <SenateStateMap
                swingStates={toSwingStates(data)}
                selectedFips={selectedFips}
                onSelect={(s) => setSelectedTag(s.tag)}
              />
            </>
          )}
        </div>
        <aside className="hidden md:block">
          {selectedMarket && <TradeUi key={selectedMarket.id} market={selectedMarket} />}
        </aside>
      </div>
    </section>
  )
}
