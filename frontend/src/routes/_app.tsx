import { createFileRoute, Link, Outlet } from '@tanstack/react-router'
import logo from '../assets/logo.png'
import { useQuery } from '@tanstack/react-query'
import { LoginPrompt } from '../components/LoginPrompt'
import { currentUserQueryOptions } from '../lib/api'

export const Route = createFileRoute('/_app')({
  component: AppLayout,
})

function AppLayout() {
  const { data: user, isPending } = useQuery(currentUserQueryOptions)

  return (
    <div className="min-h-screen">
      {user && <Navbar />}
      <main className="mx-auto max-w-7xl p-4">
        {isPending ? null : user ? <Outlet /> : <LoginPrompt />}
      </main>
    </div>
  )
}

// Placeholder nav: swap in real links and the user/balance once those pages exist.
function Navbar() {
  const { data: user } = useQuery(currentUserQueryOptions);

  return (
    <header className="border-b border-gray-200">
      <nav className="mx-auto flex max-w-7xl items-center gap-6 p-4">
        <Link to="/">
          <img src={logo} alt="Catshi" className="h-14" />
        </Link>
        <Link
          to="/election-2026"
          className="text-gray-700 hover:text-black [&.active]:font-semibold [&.active]:text-black"
        >
          Election 2026
        </Link>
        <span className="ml-auto text-gray-500">{user?.name}</span>
      </nav>
    </header>
  )
}
