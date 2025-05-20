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
    console.log('Deposit shares', deposit.shares)
    console.log('awaiting 2 seconds before redeeming')
    await new Promise(resolve => setTimeout(resolve, 2000))
    console.log('redeeming position')
    
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
      { address: david.account.address.toString() })

    expect(result).toBeDefined()
    expect(result.account.positions.length).toBe(1)

    // fully redeem the position
    console.log('Shares to redeem', result.account.positions[0].shares)
    const redemtion = await david.multivault.redeemAtom(davidAccount.vaultId, BigInt(result.account.positions[0].shares))
    expect(redemtion).toBeDefined()
    await wait(redemtion.hash)

    const result2 = await execute(
      positionsQuery,
      { address: david.account.address.toString() })

    expect(result2).toBeDefined()
    expect(result2.account.positions.length).toBe(1)
    expect(result2.account.positions[0].shares).toBe('0')

  })
})