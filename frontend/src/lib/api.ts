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

export async function redeemLoginToken(token: string): Promise<AuthUser> {
  const res = await apiFetch(`/auth/redeem/${encodeURIComponent(token)}/`, {
    method: 'POST',
  })
  return res.json()
}
