import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'
import { parseEther } from 'viem'

suite('signals', () => {
  test('should create a signals for atom deposits and redemptions', async () => {
    const user202 = await getIntuition(202)

    const barAtom = await user202.getOrCreateAtom('bar')
    await wait(barAtom.hash)
    expect(barAtom.vaultId).toBeDefined()

    const user203 = await getIntuition(203)
    console.log(user203.account.address.toString())

    const deposit = await user203.multivault.depositAtom(barAtom.vaultId, parseEther('0.05'))
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

    const res = await user203.multivault.getVaultStateForUser(barAtom.vaultId, user203.account.address)
    expect(res.shares).toBeDefined()

    const redemtion = await user203.multivault.redeemAtom(barAtom.vaultId, BigInt(res.shares))
    expect(redemtion).toBeDefined()
    await wait(redemtion.hash)


    const result2 = await execute(
      signalsQuery,
      { atom_id: barAtom.vaultId.toString() })

    expect(result2).toBeDefined()
    expect(BigInt(result2.signals[0].delta)).toBeLessThan(0)


  })

  test('should create a signals for triple deposits and redemptions', async () => {
    const user202 = await getIntuition(202)

    const barAtom = await user202.getOrCreateAtom('bar')
    await wait(barAtom.hash)
    const bazAtom = await user202.getOrCreateAtom('baz')
    await wait(bazAtom.hash)
    const fooAtom = await user202.getOrCreateAtom('foo')
    await wait(fooAtom.hash)

    const triple = await user202.getCreateOrDepositOnTriple(barAtom.vaultId, bazAtom.vaultId, fooAtom.vaultId)
    await wait(triple.hash)

    const signalsQuery = graphql(`
      query signals2($triple_id: numeric) {
        signals(where: {triple_id: {_eq: $triple_id}}, order_by: {block_timestamp: desc}) {
          delta
        }
      }
    `)

    const result = await execute(
      signalsQuery,
      { triple_id: triple.vaultId.toString() })

    expect(result).toBeDefined()
    expect(BigInt(result.signals[0].delta)).toBeGreaterThan(0)


    const user203 = await getIntuition(203)

    const counterTermId = await user203.multivault.getCounterIdFromTriple(triple.vaultId)
    console.log(counterTermId)
    try {
      const deposit2 = await user203.multivault.depositTriple(counterTermId, parseEther('0.001'))
      await wait(deposit2.hash)
    } catch (e) {
      console.log(e)
    }


    const result3 = await execute(
      signalsQuery,
      { triple_id: triple.vaultId.toString() })

    expect(result3).toBeDefined()
    expect(BigInt(result3.signals[0].delta)).toBeLessThan(0)
  })
})