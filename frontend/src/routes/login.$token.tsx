import { createFileRoute, redirect } from '@tanstack/react-router'
import { ApiError, currentUserQueryOptions, redeemLoginToken } from '../lib/api'

export const Route = createFileRoute('/login/$token')({
  // Tokens are single use, so redeem in beforeLoad (runs once per navigation)
  // rather than in an effect, which StrictMode would run twice.
  beforeLoad: async ({ params, context }) => {
    const user = await redeemLoginToken(params.token)
    // Redeem returns the user, so seed the cache and skip a /auth/me/ round trip.
    context.queryClient.setQueryData(currentUserQueryOptions.queryKey, user)
    throw redirect({ to: '/', replace: true })
  },
  errorComponent: LoginError,
})

function LoginError({ error }: { error: unknown }) {
  const expired = error instanceof ApiError && error.status === 404
  return (
    <p className="p-8 text-xl">
      {expired
        ? 'This login link is invalid or has already been used. Run /web in Discord for a new one.'
        : 'Login failed. Please try again.'}
    </p>
  )
}
