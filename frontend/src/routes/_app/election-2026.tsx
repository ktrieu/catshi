import { createFileRoute } from '@tanstack/react-router'
import { SenateMarginChart, type SenateMarginBucket } from '../../components/SenateMarginChart'

export const Route = createFileRoute('/_app/election-2026')({
  component: Election2026,
})

const MOCK_SENATE_MARGINS: SenateMarginBucket[] = [
  { label: 'D+6+', party: 'D', probability: 0.02 },
  { label: 'D+5', party: 'D', probability: 0.01 },
  { label: 'D+4', party: 'D', probability: 0.03 },
  { label: 'D+3', party: 'D', probability: 0.02 },
  { label: 'D+2', party: 'D', probability: 0.06 },
  { label: 'D+1', party: 'D', probability: 0.04 },
  { label: 'Even', party: 'even', probability: 0.1 },
  { label: 'R+1', party: 'R', probability: 0.05 },
  { label: 'R+2', party: 'R', probability: 0.18 },
  { label: 'R+3', party: 'R', probability: 0.08 },
  { label: 'R+4', party: 'R', probability: 0.22 },
  { label: 'R+5', party: 'R', probability: 0.07 },
  { label: 'R+6+', party: 'R', probability: 0.12 },
]

function Election2026() {
  return (
    <section className="py-6">
      <h1 className="text-3xl font-bold mb-6">ELECTION 2026</h1>
      <SenateMarginChart data={MOCK_SENATE_MARGINS} />
    </section>
  )
}
