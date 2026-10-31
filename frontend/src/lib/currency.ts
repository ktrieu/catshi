const BIPS_PER_YP = 1000

export function formatCurrency(bips: number): string {
  const positive = Math.abs(bips)
  const fractional = positive % BIPS_PER_YP
  const yp = (positive - fractional) / BIPS_PER_YP
  const negSign = bips < 0 ? '-' : ''

  return `${negSign}${yp}.${String(fractional).padStart(3, '0')}yp`
}
