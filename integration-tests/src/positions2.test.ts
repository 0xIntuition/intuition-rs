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

    const depositHash = await felix.contract.write.deposit(
      [felix.account.address, fooAtom.vaultId, 1n, 0n],
      { value: parseEther('0.5') }
    )
    await wait(depositHash)

    // check felix position on-chain
    const shares = await felix.contract.read.getShares([
      felix.account.address,
      fooAtom.vaultId,
      1n
    ])
    expect(shares).toBeDefined()

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
    expect(result.account?.positions.length).toBe(1)
    expect(result.account?.positions[0].shares).toBe(shares.toString())

    // fully redeem the position

    const redemtionHash = await felix.contract.write.redeem(
      [felix.account.address, fooAtom.vaultId, 1n, BigInt(result.account?.positions[0].shares), 0n]
    )
    expect(redemtionHash).toBeDefined()
    await wait(redemtionHash)

    // check felix position on-chain
    const shares2 = await felix.contract.read.getShares([
      felix.account.address,
      fooAtom.vaultId,
      1n
    ])
    expect(shares2).toBe(BigInt(0))

    const result2 = await execute(
      positionsQuery,
      { address: felix.account.address.toString() })

    expect(result2).toBeDefined()
    expect(result2.account?.positions.length).toBe(1)
    expect(result2.account?.positions[0].shares).toBe(shares2.toString())


  })
})
