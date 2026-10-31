import { createFileRoute, Link, Outlet } from '@tanstack/react-router'
import logo from '../assets/logo.png'
import { useQuery } from '@tanstack/react-query'
import { currentUserQueryOptions } from '../lib/api'

export const Route = createFileRoute('/_app')({
  component: AppLayout,
})

function AppLayout() {
  return (
    <div className="min-h-screen">
      <Navbar />
      <main className="mx-auto max-w-5xl p-4">
        <Outlet />
      </main>
    </div>
  )
}

// Placeholder nav: swap in real links and the user/balance once those pages exist.
function Navbar() {
  const { data: user } = useQuery(currentUserQueryOptions);

  return (
    <header className="border-b border-gray-200">
      <nav className="mx-auto flex max-w-5xl items-center gap-6 p-4">
        <Link to="/">
          <img src={logo} alt="Catshi" className="h-14" />
        </Link>
        <span className="text-gray-500">Markets</span>
        <span className="text-gray-500">Portfolio</span>
        <span className="ml-auto text-gray-500">{user?.name}</span>
      </nav>
    </header>
  )
}
