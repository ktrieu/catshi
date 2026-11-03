import { createFileRoute } from '@tanstack/react-router'
import { SenateMarginChart, type SenateMarginBucket } from '../../components/SenateMarginChart'
import { SenateStateMap, type SwingStateOdds } from '../../components/SenateStateMap'

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

const MOCK_SWING_STATES: SwingStateOdds[] = [
  { fips: '37', name: 'North Carolina', demProbability: 0.58 },
  { fips: '23', name: 'Maine', demProbability: 0.52 },
  { fips: '26', name: 'Michigan', demProbability: 0.55 },
  { fips: '39', name: 'Ohio', demProbability: 0.41 },
  { fips: '19', name: 'Iowa', demProbability: 0.38 },
  { fips: '48', name: 'Texas', demProbability: 0.31 },
  { fips: '02', name: 'Alaska', demProbability: 0.46 },
  { fips: '45', name: 'South Carolina', demProbability: 0.27 },
  { fips: '31', name: 'Nebraska', demProbability: 0.34 },
]

function Election2026() {
  return (
    <section className="py-6">
      <h1 className="text-3xl font-bold mb-6">ELECTION 2026</h1>
      <h3 className="text-2xl font-bold mb-6">SENATE</h3>
      <SenateMarginChart data={MOCK_SENATE_MARGINS} />
      <SenateStateMap swingStates={MOCK_SWING_STATES} />
    </section>
  )
}
