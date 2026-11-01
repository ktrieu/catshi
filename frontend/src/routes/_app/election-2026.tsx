import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/_app/election-2026')({
  component: Election2026,
})

function Election2026() {
  return (
    <section className="py-6">
      <h1 className="text-3xl font-bold">ELECTION 2026</h1>
    </section>
  )
}
