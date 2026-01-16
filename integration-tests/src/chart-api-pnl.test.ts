import { expect, suite, test } from 'vitest'

const baseUrl = process.env.CHART_API_URL
const chartSuite = baseUrl ? suite : suite.skip

chartSuite('chart api pnl validation', () => {
  test('rejects invalid account id', async () => {
    const url = new URL('/api/v1/accounts/not-an-address/pnl', baseUrl!)
    url.searchParams.set('interval', '1d')
    url.searchParams.set('start', '2026-01-01T00:00:00Z')
    url.searchParams.set('end', '2026-01-02T00:00:00Z')

    const response = await fetch(url.toString())
    expect(response.status).toBe(400)

    const payload = await response.json()
    expect(payload.code).toBe('INVALID_ACCOUNT_ID')
  })

  test('rejects invalid term id before querying data', async () => {
    const accountId = '0x0123456789abcdef0123456789abcdef01234567'
    const url = new URL(
      `/api/v1/accounts/${accountId}/positions/1234/1/pnl`,
      baseUrl!,
    )
    url.searchParams.set('interval', '1d')
    url.searchParams.set('start', '2026-01-01T00:00:00Z')
    url.searchParams.set('end', '2026-01-02T00:00:00Z')

    const response = await fetch(url.toString())
    expect(response.status).toBe(400)

    const payload = await response.json()
    expect(payload.code).toBe('INVALID_TERM_ID')
  })
})
