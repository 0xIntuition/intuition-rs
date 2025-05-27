import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'
import { parseEther } from 'viem'

suite('signals', () => {
  test('should create a signal', async () => {
    const user202 = await getIntuition(202)

    const barAtom = await user202.getOrCreateAtom('bar')
    await wait(barAtom.hash)
    expect(barAtom.vaultId).toBeDefined()

    const user201 = await getIntuition(201)
    console.log(user201.account.address.toString())

    const deposit = await user201.multivault.depositAtom(barAtom.vaultId, parseEther('0.05'))
    await wait(deposit.hash)

    const signalsQuery = graphql(`
      query signals($atom_id: numeric) {
        signals(where: {atom_id: {_eq: $atom_id}}, order_by: {block_timestamp: desc}) {
          delta
        }
      }
    `)

    const result = await execute(
      signalsQuery,
      { atom_id: barAtom.vaultId.toString() })

    expect(result).toBeDefined()
    expect(BigInt(result.signals[0].delta)).toBeGreaterThan(0)

    // fully redeem the position

    const res = await user201.multivault.getVaultStateForUser(barAtom.vaultId, user201.account.address)
    expect(res.shares).toBeDefined()

    const redemtion = await user201.multivault.redeemAtom(barAtom.vaultId, BigInt(res.shares))
    expect(redemtion).toBeDefined()
    await wait(redemtion.hash)


    const result2 = await execute(
      signalsQuery,
      { atom_id: barAtom.vaultId.toString() })

    expect(result2).toBeDefined()
    expect(BigInt(result2.signals[0].delta)).toBeLessThan(0)


  })
})