import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'
import { parseEther } from 'viem'

suite('positions2', () => {
  test('should deposit and full redeem on existing atom', async () => {
    const evin = await getIntuition(200)

    const fooAtom = await evin.getOrCreateAtom('foo')
    await wait(fooAtom.hash)
    expect(fooAtom.vaultId).toBeDefined()

    const felix = await getIntuition(201)
    console.log(felix.account.address.toString())

    const deposit = await felix.multivault.depositAtom(fooAtom.vaultId, parseEther('0.05'))
    await wait(deposit.hash)

    // check felix position on-chain
    const res = await felix.multivault.getVaultStateForUser(fooAtom.vaultId, felix.account.address)
    expect(res.shares).toBeDefined()

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
      { address: felix.account.address.toString() })

    expect(result).toBeDefined()
    expect(result.account.positions.length).toBe(1)
    expect(result.account.positions[0].shares).toBe(res.shares.toString())

    // fully redeem the position

    const redemtion = await felix.multivault.redeemAtom(fooAtom.vaultId, BigInt(result.account.positions[0].shares))
    expect(redemtion).toBeDefined()
    await wait(redemtion.hash)

    // check felix position on-chain
    const res2 = await felix.multivault.getVaultStateForUser(fooAtom.vaultId, felix.account.address)
    expect(res2.shares).toBe(BigInt(0))

    const result2 = await execute(
      positionsQuery,
      { address: felix.account.address.toString() })

    expect(result2).toBeDefined()
    expect(result2.account.positions.length).toBe(1)
    expect(result2.account.positions[0].shares).toBe(res2.shares.toString())


  })
})