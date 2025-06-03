import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
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
      { address: user301.account.address.toString(), term_id: counterVault.toString(), curve_id: '1' })

    expect(result).toBeDefined()
    expect(result.positions.length).toBe(1)
    expect(result.positions[0].shares).toBe(user301State.shares.toString())
    expect(result.positions[0].curve_id).toBe('1')
    expect(result.positions[0].term_id).toBe(counterVault.toString())
  })
})