import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

suite('random atom', () => {
  test('create atom with initial deposit', async () => {
    const randomId = Math.floor(Math.random() * 1000000)

    const user = await getIntuition(randomId)

    const userAtom = await user.getOrCreateAtom(user.account.address)
    await wait(userAtom.hash)
    expect(userAtom.vaultId).toBeDefined()

    const result = await execute(
      graphql(`query Atom($term_id: String!) {
        atom(term_id: $term_id) {
          data
          label
          term_id
          resolving_status
          creator_id
          type
          term {
            type
            vaults {
              curve_id
              market_cap
              position_count
              total_assets
              total_shares
              positions {
                account_id
                shares
                total_deposit_assets_after_total_fees
                total_redeem_assets_for_receiver
              }
            }
            deposits {
              curve_id
              receiver_id
              sender_id
              shares
              total_shares
              assets_after_fees
              vault_type
            }
            share_price_changes{
              curve_id
              share_price
              total_assets
              total_shares
            }
          }
        }
      }`),
      { term_id: userAtom.vaultId })

    // Validate atom metadata
    expect(result).toBeDefined()
    expect(result.atom).toBeDefined()
    expect(result.atom?.data).toBe(user.account.address)
    expect(result.atom?.term_id).toBe(userAtom.vaultId)
    expect(result.atom?.type).toBe('Account')

    // Validate term structure
    expect(result.atom?.term).toBeDefined()
    expect(result.atom?.term.type).toBe('Atom')

    // Validate vaults
    expect(result.atom?.term.vaults).toBeDefined()
    expect(result.atom?.term.vaults.length).toBe(1)

    const vault = result.atom?.term.vaults[0]
    expect(vault?.curve_id).toBe('1')
    expect(vault?.position_count).toBe(1)
    expect(vault?.total_assets).toBe('980000001000000')
    expect(vault?.total_shares).toBe('980000001000000')
    expect(vault?.market_cap).toBe('980000001000000')

    // Validate positions
    expect(vault?.positions).toBeDefined()
    expect(vault?.positions.length).toBe(1)

    const position = vault?.positions[0]
    expect(position?.account_id).toBe(user.account.address)
    expect(position?.shares).toBe('980000000000000')
    expect(position?.total_deposit_assets_after_total_fees).toBe('980000000000000')
    expect(position?.total_redeem_assets_for_receiver).toBe('0')

    // Validate deposits
    expect(result.atom?.term.deposits).toBeDefined()
    expect(result.atom?.term.deposits.length).toBeGreaterThan(0)

    const deposit = result.atom?.term.deposits[0]
    expect(deposit?.curve_id).toBe('1')
    expect(deposit?.sender_id).toBe(user.account.address)
    expect(deposit?.receiver_id).toBe(user.account.address)
    expect(deposit?.shares).toBe('980000000000000')
    expect(deposit?.assets_after_fees).toBe('980000000000000')
    expect(deposit?.vault_type).toBe('Atom')

    // Validate share price changes
    expect(result.atom?.term.share_price_changes).toBeDefined()
    expect(result.atom?.term.share_price_changes.length).toBeGreaterThan(0)

    const sharePriceChange = result.atom?.term.share_price_changes[0]
    expect(sharePriceChange?.curve_id).toBe('1')
    expect(sharePriceChange?.total_assets).toBe('980000001000000')
    expect(sharePriceChange?.total_shares).toBe('980000001000000')


  })
})
