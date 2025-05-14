import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'
import { parseEther } from 'viem'

suite('positions', () => {
  test('should create and redeem a position', async () => {
    const david = await getIntuition(100)

    const davidAccount = await david.getOrCreateAtom(david.account.address)
    await wait(davidAccount.hash)
    expect(davidAccount).toBeDefined()

    // deposit 0.05 ETH to david
    const deposit = await david.multivault.depositAtom(davidAccount.vaultId, parseEther('0.05'))
    await wait(deposit.hash)

    const positionsQuery = graphql(`
      query positions($address: String!) {
        account(id: $address) {
          positions {
            id
            curve_id
            term_id
            shares
          }
        }
      }
    `)

    const result = await execute(
      positionsQuery,
      { address: david.account.address.toString().toLowerCase() })

    expect(result).toBeDefined()
    expect(result.account.positions.length).toBe(1)

    // fully redeem the position

    const redemtion = await david.multivault.redeemAtom(davidAccount.vaultId, BigInt(result.account.positions[0].shares))
    expect(redemtion).toBeDefined()
    await wait(redemtion.hash)

    const result2 = await execute(
      positionsQuery,
      { address: david.account.address.toString().toLowerCase() })

    expect(result2).toBeDefined()
    expect(result2.account.positions.length).toBe(1)
    expect(result2.account.positions[0].shares).toBe('0')

  })
})