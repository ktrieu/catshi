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
    throw new ApiError(res.status, `${res.status} ${res.statusText}`)
  }
  return res
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
