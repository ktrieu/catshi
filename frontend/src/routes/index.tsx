import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/')({
  component: Index,
})

function Index() {
  return (
    <p className="text-8xl">
      🐈🐟
    </p>
  )
}
