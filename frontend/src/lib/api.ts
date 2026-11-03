import { queryOptions } from '@tanstack/react-query'

// All API calls go through /api, which the dev server proxies to the backend.
// Keeping it same-origin means the session cookie needs no CORS setup.
export class ApiError extends Error {
  status: number

  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

export async function apiFetch(path: string, init?: RequestInit): Promise<Response> {
  const res = await fetch(`/api${path}`, { credentials: 'include', ...init })
  if (!res.ok) {
    let message = `${res.status} ${res.statusText}`
    try {
      const body = await res.json()
      if (typeof body?.error === 'string' && body.error) message = body.error
    } catch {
      message = `${res.status} ${res.statusText}`
    }
    throw new ApiError(res.status, message)
  }
  return res
}

export interface TradeParams {
  quantity: number
  direction: 'buy' | 'sell'
  market_id: number
  instrument_id: number
}

export interface TradeResult {
  user: AuthUser
  market_id: number
  instrument_id: number
  quantity: number
  shares_price: number
  fees: number
  total: number
  position_quantity: number
}

export async function placeTrade(params: TradeParams): Promise<TradeResult> {
  const res = await apiFetch('/trade/', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(params),
  })
  return res.json()
}

export type QuoteParams = TradeParams

export interface TradeQuote {
  market_id: number
  instrument_id: number
  quantity: number
  shares_price: number
  fees: number
  total: number
  max_quantity: number
}

export async function getTradeQuote(params: QuoteParams): Promise<TradeQuote> {
  const search = new URLSearchParams({
    quantity: String(params.quantity),
    direction: params.direction,
    market_id: String(params.market_id),
    instrument_id: String(params.instrument_id),
  })
  const res = await apiFetch(`/trade/quote/?${search}`)
  return res.json()
}

export interface AuthUser {
  id: number
  discord_id: string
  name: string
  // Integer 1000ths of a YP.
  cash_balance: number
}

export interface Portfolio {
  user: AuthUser
  net_deposits: number
  trades_profit: number
  fees_profit: number
  gambling_winnings: number
  tips_received: number
  tips_sent: number
  transfers_received: number
  transfers_sent: number
  positions_value: number
  net_profit: number
  net_worth: number
}

export const portfolioQueryOptions = queryOptions({
  queryKey: ['portfolio'],
  queryFn: async (): Promise<Portfolio> => {
    const res = await apiFetch('/portfolio/')
    return res.json()
  },
})

export async function redeemLoginToken(token: string): Promise<AuthUser> {
  const res = await apiFetch(`/auth/redeem/${encodeURIComponent(token)}/`, {
    method: 'POST',
  })
  return res.json()
}

// The API answers 403 when there is no session; treat that as "logged out"
// (null) rather than an error so consumers don't have to special-case it.
export const currentUserQueryOptions = queryOptions({
  queryKey: ['auth', 'me'],
  queryFn: async (): Promise<AuthUser | null> => {
    try {
      const res = await apiFetch('/auth/me/')
      return await res.json()
    } catch (e) {
      if (e instanceof ApiError && e.status === 403) return null
      throw e
    }
  },
})

export interface ApiPosition {
  id: number
  owner_id: number
  owner_name: string
  quantity: number
  // Integer 1000ths of a YP.
  cost_basis: number
}

export interface ApiInstrument {
  id: number
  name: string
  state: 'open' | 'winner' | 'loser'
  shares: number
  // Integer 1000ths of a YP; a probability out of 1000.
  price: number
  positions: ApiPosition[]
}

export interface ApiMarket {
  id: number
  description: string
  state: 'open' | 'closed'
  owner_id: number
  owner_name: string
  instruments: ApiInstrument[]
}

export type Elxn26Markets = Record<string, ApiMarket>

export const elxn26MarketsQueryOptions = queryOptions({
  queryKey: ['elxn26', 'markets'],
  queryFn: async (): Promise<Elxn26Markets> => {
    const res = await apiFetch('/elxn26/markets/')
    return res.json()
  },
})
