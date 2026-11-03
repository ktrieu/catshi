import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { portfolioQueryOptions } from '../../lib/api'
import { formatCurrency } from '../../lib/currency'

export const Route = createFileRoute('/_app/')({
  component: Home,
})

function Home() {
  const { data: portfolio } = useQuery(portfolioQueryOptions)

  return (
    <>
      <section className="py-6">
        <p className="text-9xl font-bold">
          {portfolio ? formatCurrency(portfolio.net_worth) : '—'}
        </p>
      </section>
    </>
  )
}
