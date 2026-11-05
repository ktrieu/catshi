import { useQuery } from '@tanstack/react-query'
import { useState } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { marketsQueryOptions, portfolioQueryOptions } from '../../lib/api'
import { formatCurrency } from '../../lib/currency'

export const Route = createFileRoute('/_app/')({
  head: () => ({ meta: [{ title: 'Catshi' }] }),
  component: Home,
})

function Home() {
  const { data: portfolio } = useQuery(portfolioQueryOptions)
  const { data: markets } = useQuery(
    marketsQueryOptions({ status: 'open', staked: true }),
  )
  const userId = portfolio?.user.id
  const [openIds, setOpenIds] = useState<Set<number>>(new Set())
  const allOpen =
    !!markets?.length && markets.every((m) => openIds.has(m.id))

  const setMarketOpen = (id: number, open: boolean) =>
    setOpenIds((prev) => {
      if (prev.has(id) === open) return prev
      const next = new Set(prev)
      if (open) next.add(id)
      else next.delete(id)
      return next
    })

  return (
    <>
      <section className="py-6">
        <p className="text-5xl sm:text-7xl lg:text-9xl font-bold">
          {portfolio ? formatCurrency(portfolio.net_worth) : '—'}
        </p>
      </section>
      <section className="py-6">
        <div className="mb-4 flex items-center justify-between gap-4">
          <h2 className="text-2xl font-bold">Your open markets</h2>
          {!!markets?.length && (
            <button
              type="button"
              className="cursor-pointer text-sm underline"
              onClick={() =>
                setOpenIds(
                  allOpen ? new Set() : new Set(markets.map((m) => m.id)),
                )
              }
            >
              {allOpen ? 'Collapse all' : 'Expand all'}
            </button>
          )}
        </div>
        {markets?.length === 0 && <p>No open positions.</p>}
        <ul className="space-y-3">
          {markets?.map((market) => {
            let value = 0
            let costBasis = 0
            for (const instrument of market.instruments) {
              for (const p of instrument.positions) {
                if (p.owner_id !== userId) continue
                value += p.quantity * instrument.price
                costBasis += p.cost_basis
              }
            }
            const profitPct =
              costBasis > 0 ? ((value - costBasis) / costBasis) * 100 : null

            return (
              <li key={market.id}>
                <details
                  className="group rounded-lg border shadow-sm"
                  open={openIds.has(market.id)}
                  onToggle={(e) =>
                    setMarketOpen(market.id, e.currentTarget.open)
                  }
                >
                  <summary className="flex cursor-pointer list-none items-center gap-4 p-4 [&::-webkit-details-marker]:hidden">
                    <span className="shrink-0 transition-transform group-open:rotate-90">
                      ▸
                    </span>
                    <span className="shrink-0 tabular-nums">#{market.id}</span>
                    <span className="min-w-0 flex-1 truncate font-bold">
                      {market.description}
                    </span>
                    <span
                      className={`w-20 shrink-0 text-right tabular-nums ${
                        profitPct === null
                          ? ''
                          : profitPct >= 0
                            ? 'text-green-600'
                            : 'text-red-600'
                      }`}
                    >
                      {profitPct === null
                        ? '—'
                        : `${profitPct >= 0 ? '+' : ''}${profitPct.toFixed(1)}%`}
                    </span>
                  </summary>
                  <ul className="space-y-1 border-t px-4 py-3">
                    {market.instruments.map((instrument) => {
                      const held = instrument.positions.find(
                        (p) => p.owner_id === userId && p.quantity > 0,
                      )

                      return (
                        <li
                          key={instrument.id}
                          className={`flex items-center gap-4 ${
                            held ? 'text-blue-600' : ''
                          }`}
                        >
                          <span className="min-w-0 flex-1 truncate">
                            {instrument.name}
                          </span>
                          <span className="w-16 shrink-0 text-right tabular-nums">
                            {held ? `×${held.quantity}` : ''}
                          </span>
                          <span className="shrink-0 tabular-nums">
                            {formatCurrency(instrument.price)}
                          </span>
                        </li>
                      )
                    })}
                  </ul>
                </details>
              </li>
            )
          })}
        </ul>
      </section>
    </>
  )
}
