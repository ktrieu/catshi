import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { currentUserQueryOptions } from '../lib/api'

export const Route = createFileRoute('/')({
  component: Index,
})

function Index() {
  const { data: user, error, isPending } = useQuery(currentUserQueryOptions)

  return (
    <>
      <p className="text-8xl">
        🐈🐟
      </p>
      {/* Debug view: remove once there's a real user UI. */}
      <pre className="p-4 text-sm">
        {isPending
          ? 'loading...'
          : error
            ? `error: ${error.message}`
            : user
              ? JSON.stringify(user, null, 2)
              : 'not logged in'}
      </pre>
    </>
  )
}
