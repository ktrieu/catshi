import { useState } from 'react'
import { formatCurrency } from '../lib/currency'
import type { ApiInstrument, ApiMarket } from '../lib/api'

type Side = 'buy' | 'sell'

interface Selection {
  instrument: ApiInstrument
  side: Side
}

export function TradeUi({ market }: { market: ApiMarket }) {
  const [selection, setSelection] = useState<Selection | null>(null)
  const [quantity, setQuantity] = useState('1')

  const select = (instrument: ApiInstrument, side: Side) => {
    setSelection({ instrument, side })
    setQuantity('1')
  }

  const parsedQuantity = Number.parseInt(quantity, 10)
  const validQuantity = Number.isFinite(parsedQuantity) && parsedQuantity > 0
  const total = validQuantity ? parsedQuantity : 0

  return (
    <div className="flex flex-col rounded-lg border border-gray-200">
      <div className="border-b border-gray-200 p-4">
        <p className="font-semibold">{market.description}</p>
        <p className="text-sm text-gray-500">Owner: {market.owner_name}</p>
      </div>
      <ul className="divide-y divide-gray-100">
        {market.instruments.map((instrument) => (
          <li key={instrument.id} className="flex items-center gap-3 p-3">
            <div className="min-w-0 flex-1">
              <p className="truncate">{instrument.name}</p>
              <p className="text-sm text-gray-500">{formatCurrency(instrument.price)}</p>
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
          </li>
        ))}
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
                  onClick={() => setSelection(null)}
                  className="text-sm text-gray-500 hover:text-black"
                >
                  Cancel
                </button>
              </div>
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
                disabled={!validQuantity}
                className="w-full rounded bg-black px-3 py-2 font-medium text-white hover:bg-gray-800 disabled:opacity-40"
              >
                Confirm
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
