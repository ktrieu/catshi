import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { formatCurrency } from '../lib/currency'
import { currentUserQueryOptions, placeTrade } from '../lib/api'
import type { ApiInstrument, ApiMarket, TradeResult } from '../lib/api'

type Side = 'buy' | 'sell'

interface Selection {
  instrument: ApiInstrument
  side: Side
}

export function TradeUi({ market }: { market: ApiMarket }) {
  const queryClient = useQueryClient()
  const { data: user } = useQuery(currentUserQueryOptions)
  const [selection, setSelection] = useState<Selection | null>(null)
  const [quantity, setQuantity] = useState('1')

  const trade = useMutation({
    mutationFn: placeTrade,
    onSuccess: (result: TradeResult) => {
      queryClient.setQueryData(['auth', 'me'], result.user)
      queryClient.invalidateQueries({ queryKey: ['portfolio'] })
      queryClient.invalidateQueries({ queryKey: ['elxn26', 'markets'] })
    },
  })

  const [expanded, setExpanded] = useState<Set<number>>(new Set())

  const toggleExpanded = (id: number) => {
    setExpanded((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const allExpanded =
    market.instruments.length > 0 && market.instruments.every((i) => expanded.has(i.id))

  const toggleAll = () => {
    setExpanded(allExpanded ? new Set() : new Set(market.instruments.map((i) => i.id)))
  }

  const select = (instrument: ApiInstrument, side: Side) => {
    trade.reset()
    setSelection({ instrument, side })
    setQuantity('1')
  }

  const cancel = () => {
    trade.reset()
    setSelection(null)
  }

  const confirm = () => {
    if (!selection) return
    trade.mutate({
      quantity: Number.parseInt(quantity, 10),
      direction: selection.side,
      market_id: market.id,
      instrument_id: selection.instrument.id,
    })
  }

  const parsedQuantity = Number.parseInt(quantity, 10)
  const validQuantity = Number.isFinite(parsedQuantity) && parsedQuantity > 0
  const total = validQuantity ? parsedQuantity : 0

  return (
    <div className="flex flex-col rounded-lg border border-gray-200">
      <div className="flex items-start justify-between gap-3 border-b border-gray-200 p-4">
        <div className="min-w-0">
          <p className="font-semibold">{market.description}</p>
          <p className="text-sm text-gray-500">Owner: {market.owner_name}</p>
        </div>
        <button
          type="button"
          onClick={toggleAll}
          className="shrink-0 text-sm text-gray-500 hover:text-black"
        >
          {allExpanded ? 'Collapse all' : 'Expand all'}
        </button>
      </div>
      <ul className="divide-y divide-gray-100">
        {market.instruments.map((instrument) => {
          const mine = instrument.positions.find((p) => p.owner_id === user?.id)
          return (
          <li key={instrument.id} className={mine ? 'bg-blue-50' : ''}>
            <div className="flex items-center gap-3 p-3">
              <div className="min-w-0 flex-1">
                <p className="truncate">{instrument.name}</p>
                <p className="text-sm text-gray-500">
                  {formatCurrency(instrument.price)}
                  {' · '}
                  <button
                    type="button"
                    onClick={() => toggleExpanded(instrument.id)}
                    aria-expanded={expanded.has(instrument.id)}
                    className="inline-flex items-center gap-1 hover:text-black"
                  >
                    {instrument.positions.length}{' '}
                    {instrument.positions.length === 1 ? 'owner' : 'owners'}
                    <span
                      className={`inline-block transition-transform ${expanded.has(instrument.id) ? 'rotate-180' : ''}`}
                    >
                      ▾
                    </span>
                  </button>
                </p>
                {mine && (
                  <p className="text-sm font-medium text-blue-600">{mine.quantity} owned</p>
                )}
              </div>
              <button
                type="button"
                onClick={() => select(instrument, 'buy')}
                className="rounded bg-green-600 px-3 py-1 text-sm font-medium text-white hover:bg-green-700"
              >
                Buy
              </button>
              <button
                type="button"
                onClick={() => select(instrument, 'sell')}
                className="rounded bg-red-600 px-3 py-1 text-sm font-medium text-white hover:bg-red-700"
              >
                Sell
              </button>
            </div>
            <div
              className={`grid transition-all duration-200 ${expanded.has(instrument.id) ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'}`}
            >
              <div className="overflow-hidden">
                {instrument.positions.length === 0 ? (
                  <p className="px-3 pb-3 text-sm text-gray-500">No owners yet</p>
                ) : (
                  <ul className="space-y-1 px-3 pb-3 text-sm">
                    {[...instrument.positions]
                      .sort((a, b) => b.quantity - a.quantity)
                      .map((p) => (
                        <li
                          key={p.id}
                          className={`flex justify-between ${p.owner_id === user?.id ? 'font-medium text-blue-600' : 'text-gray-700'}`}
                        >
                          <span className="truncate">
                            {p.owner_name}
                            {p.owner_id === user?.id && ' (you)'}
                          </span>
                          <span>{p.quantity}</span>
                        </li>
                      ))}
                  </ul>
                )}
              </div>
            </div>
          </li>
          )
        })}
      </ul>
      <div
        className={`grid transition-all duration-200 ${selection ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'}`}
      >
        <div className="overflow-hidden">
          {selection && (
            <div className="space-y-3 border-t border-gray-200 p-4">
              <div className="flex items-center justify-between">
                <p className="font-semibold">
                  {selection.side === 'buy' ? 'Buy' : 'Sell'} {selection.instrument.name}
                </p>
                <button
                  type="button"
                  onClick={cancel}
                  className="text-sm text-gray-500 hover:text-black"
                >
                  {trade.isSuccess ? 'Close' : 'Cancel'}
                </button>
              </div>
              {trade.isSuccess ? (
                <p className="rounded bg-green-50 p-3 text-sm font-medium text-green-700">
                  Trade complete
                </p>
              ) : trade.isError ? (
                <p className="rounded bg-red-50 p-3 text-sm font-medium text-red-700">
                  {trade.error.message}
                </p>
              ) : (
                <>
                  <label className="block text-sm text-gray-700">
                    Quantity
                    <input
                      type="number"
                      min={1}
                      step={1}
                      value={quantity}
                      onChange={(e) => setQuantity(e.target.value)}
                      className="mt-1 block w-full rounded border border-gray-300 px-2 py-1"
                    />
                  </label>
                  <p className="flex justify-between text-sm">
                    <span className="text-gray-700">Price</span>
                    <span className="font-semibold">{formatCurrency(total * 1000)}</span>
                  </p>
                  <button
                    type="button"
                    disabled={!validQuantity || trade.isPending}
                    onClick={confirm}
                    className="w-full rounded bg-black px-3 py-2 font-medium text-white hover:bg-gray-800 disabled:opacity-40"
                  >
                    {trade.isPending ? 'Confirming...' : 'Confirm'}
                  </button>
                </>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
