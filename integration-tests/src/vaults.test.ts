import { expect, test, suite } from 'vitest'
import { execute, getIntuition, getCounterVaultId, wait, oxToBackslashX } from './setup/utils.js'
import { graphql } from './graphql/gql.js'
import { Address, parseEther } from 'viem'

suite('vaults', () => {

  let triple: { vaultId: `0x${string}`, hash: `0x${string}` | null }
  let counterVault: `0x${string}`
  test('counter vault should be initialized', async () => {
    const user300 = await getIntuition(300)

    triple = await user300.getCreateOrDepositOnTriple(
      (await user300.getOrCreateAtom('foo')).vaultId,
      (await user300.getOrCreateAtom('bar')).vaultId,
      (await user300.getOrCreateAtom('baz')).vaultId,
    )
    console.log({ triple })

    counterVault = await user300.contract.read.getCounterIdFromTripleId([triple.vaultId])
    console.log({ counterVault })
    expect(counterVault).toBeDefined()

    const counterVaultState = await user300.contract.read.vaults([counterVault, 1n])
    expect(counterVaultState[0]).toBeDefined()
    // can't check for 1000000 becase this test can run multiple times
    expect(counterVaultState[0]).toBeGreaterThan(BigInt(0))
    expect(counterVaultState[1]).toBeDefined()
    expect(counterVaultState[1]).toBeGreaterThan(BigInt(0))
  })

  test('deposit on counter vault should work', async () => {
    const user301 = await getIntuition(301)

    const hash = await user301.contract.write.deposit([user301.account.address, counterVault, 1n, 0n], { value: parseEther('0.01') })
    await wait(hash)

    const counterVaultState = await user301.contract.read.vaults([counterVault, 1n])
    expect(counterVaultState[0]).toBeDefined()
    // can't check for 1000000 becase this test can run multiple times
    expect(counterVaultState[0]).toBeGreaterThan(BigInt(0))
    expect(counterVaultState[1]).toBeDefined()
    expect(counterVaultState[1]).toBeGreaterThan(BigInt(0))


    const shares = await user301.contract.read.maxRedeem([
      user301.account.address,
      counterVault,
      1n
    ])
    expect(shares).toBeDefined()


    const positionsQuery = graphql(`
      query positions2($address: String!, $term_id: bytea!, $curve_id: numeric!) {
        positions(where: {account_id: {_eq: $address}, curve_id: {_eq: $curve_id}, term_id: {_eq: $term_id}}) {
          id
          curve_id
          term_id
          shares
        }
      }
    `)

    const result = await execute(
      positionsQuery,
      {
        address: user301.account.address.toString(),
        term_id: counterVault,
        curve_id: '1'
      })

    expect(result).toBeDefined()
    expect(result.positions.length).toBe(1)
    expect(result.positions[0].shares).toBe(shares.toString())
    expect(result.positions[0].curve_id).toBe('1')
    expect(result.positions[0].term_id).toBe(counterVault.toString())
  })

  test('misc signals on a triple', async () => {
    const user351 = await getIntuition(351)

    const signal1 = await user351.contract.write.deposit(
      [user351.account.address, counterVault, 1n, 0n],
      { value: parseEther('0.1') }
    )
    await wait(signal1)

    const signal2 = await user351.contract.write.redeem(
      [user351.account.address, counterVault, 1n, parseEther('0.001'), 0n]
    )
    await wait(signal2)

    const user352 = await getIntuition(352)

    const signal3 = await user352.contract.write.deposit(
      [user352.account.address, triple.vaultId, 1n, 0n],
      { value: parseEther('0.1') }
    )
    await wait(signal3)

    const signal4 = await user352.contract.write.redeem(
      [user352.account.address, triple.vaultId, 1n, parseEther('0.001'), 0n]
    )
    await wait(signal4)

    const user353 = await getIntuition(353)

    const signal5 = await user353.contract.write.deposit(
      [user353.account.address, counterVault, 1n, 0n],
      { value: parseEther('0.1') }
    )
    await wait(signal5)

    const shares = await user353.contract.read.maxRedeem([
      user353.account.address,
      counterVault,
      1n
    ])

    // full redeem
    const signal6 = await user353.contract.write.redeem(
      [user353.account.address, counterVault, 1n, shares, 0n]
    )
    await wait(signal6)

    expect(signal6).toBeDefined()
  })

  test('triple vault numbers are correct', async () => {

    const tripleQuery = graphql(`
query triple($term_id: bytea!) {
  triple(term_id: $term_id) {
    term_id
    term {
      total_assets
      total_market_cap
    }
    counter_term {
      total_assets
      total_market_cap
    }
    triple_term {
      total_assets
      total_market_cap
    }
  }
}
`)

    const result = await execute(
      tripleQuery,
      { term_id: triple.vaultId })

    expect(result).toBeDefined()
    expect(BigInt(result.triple?.triple_term?.total_assets)).toEqual(
      BigInt(result.triple?.term?.total_assets) + BigInt(result.triple?.counter_term?.total_assets)
    )
    expect(BigInt(result.triple?.triple_term?.total_market_cap)).toEqual(
      BigInt(result.triple?.term?.total_market_cap) + BigInt(result.triple?.counter_term?.total_market_cap)
    )
  })


})
