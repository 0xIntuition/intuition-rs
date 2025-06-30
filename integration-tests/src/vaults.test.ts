import { expect, test, suite } from 'vitest'
import { execute, getIntuition, getCounterVaultId, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'
import { Address, parseEther } from 'viem'

suite('vaults', () => {
  test('atom wallet should have a position on an atom', async () => {
    const user300 = await getIntuition(300)

    const fooAtom = await user300.getOrCreateAtom('foo')
    await wait(fooAtom.hash)
    expect(fooAtom.vaultId).toBeDefined()

    const { atom } = await execute(graphql(`
      query atom($id: numeric!) {
        atom(term_id: $id) {
          wallet_id
        }
      }
    `), { id: fooAtom.vaultId.toString() })
    expect(atom.wallet_id).toBeDefined()

    const res = await user300.multivault.getVaultStateForUser(
      fooAtom.vaultId,
      atom.wallet_id as Address
    )

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
      { address: atom.wallet_id.toString() })

    expect(result).toBeDefined()
    expect(result.account.positions.length).toBe(1)
    expect(result.account.positions[0].shares).toBe(res.shares.toString())

  })

  let triple: { vaultId: bigint }
  let counterVault: bigint
  test('counter vault should be initialized', async () => {
    const user300 = await getIntuition(300)

    triple = await user300.getCreateOrDepositOnTriple(
      (await user300.getOrCreateAtom('foo')).vaultId,
      (await user300.getOrCreateAtom('bar')).vaultId,
      (await user300.getOrCreateAtom('baz')).vaultId,
    )

    counterVault = await user300.multivault.getCounterIdFromTriple(triple.vaultId)
    expect(counterVault).toBeDefined()

    const counterVaultState = await user300.multivault.getVaultState(counterVault)
    expect(counterVaultState.totalAssets).toBeDefined()
    // can't check for 1000000 becase this test can run multiple times
    expect(counterVaultState.totalAssets).toBeGreaterThan(BigInt(0))
    expect(counterVaultState.totalShares).toBeDefined()
    expect(counterVaultState.totalShares).toBeGreaterThan(BigInt(0))
  })

  test('deposit on counter vault should work', async () => {
    const user301 = await getIntuition(301)

    const deposit = await user301.multivault.depositTriple(counterVault, parseEther('0.01'))
    await wait(deposit.hash)

    const counterVaultState = await user301.multivault.getVaultState(counterVault)
    expect(counterVaultState.totalAssets).toBeDefined()
    expect(counterVaultState.totalAssets).toBeGreaterThan(BigInt(1000000))
    expect(counterVaultState.totalShares).toBeDefined()
    expect(counterVaultState.totalShares).toBeGreaterThan(BigInt(1000000))


    const user301State = await user301.multivault.getVaultStateForUser(
      counterVault,
      user301.account.address
    )
    expect(user301State.shares).toBeDefined()


    const positionsQuery = graphql(`
      query positions2($address: String!, $term_id: numeric!, $curve_id: numeric!) {
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
        term_id: counterVault.toString(),
        curve_id: '1'
      })

    expect(result).toBeDefined()
    expect(result.positions.length).toBe(1)
    expect(result.positions[0].shares).toBe(user301State.shares.toString())
    expect(result.positions[0].curve_id).toBe('1')
    expect(result.positions[0].term_id).toBe(counterVault.toString())
  })

  test('misc signals on a triple', async () => {
    const user351 = await getIntuition(351)

    const signal1 = await user351.multivault.depositTriple(counterVault, parseEther('0.01'))
    await wait(signal1.hash)

    const signal2 = await user351.multivault.redeemTriple(counterVault, parseEther('0.001'))
    await wait(signal2.hash)

    const user352 = await getIntuition(352)

    const signal3 = await user352.multivault.depositTriple(triple.vaultId, parseEther('0.01'))
    await wait(signal3.hash)

    const signal4 = await user352.multivault.redeemTriple(triple.vaultId, parseEther('0.001'))
    await wait(signal4.hash)

    const user353 = await getIntuition(353)

    const signal5 = await user353.multivault.depositTriple(counterVault, parseEther('0.01'))
    await wait(signal5.hash)

    const user353State = await user353.multivault.getVaultStateForUser(
      counterVault,
      user353.account.address
    )

    // full redeem
    const signal6 = await user353.multivault.redeemTriple(counterVault, user353State.shares)
    await wait(signal6.hash)

    expect(signal6.hash).toBeDefined()
  })

  test('triple vault numbers are correct', async () => {

    const tripleQuery = graphql(`
query triple($term_id: numeric!) {
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
      { term_id: triple.vaultId.toString() })

    expect(result).toBeDefined()
    expect(BigInt(result.triple.triple_term.total_assets)).toEqual(
      BigInt(result.triple.term.total_assets) + BigInt(result.triple.counter_term.total_assets)
    )
    expect(BigInt(result.triple.triple_term.total_market_cap)).toEqual(
      BigInt(result.triple.term.total_market_cap) + BigInt(result.triple.counter_term.total_market_cap)
    )
  })

  test('computing counterVaultId', () => {
    expect(counterVault).toEqual(getCounterVaultId(triple.vaultId))
  })

})
