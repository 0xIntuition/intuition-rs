import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'
import { parseEther } from 'viem'

suite('triple terms and vaults', () => {
  test('triple term and triple vault math should be correct', async () => {
    const user1 = await getIntuition(1001)

    const atom1 = await user1.getOrCreateAtom('atom1')
    const atom2 = await user1.getOrCreateAtom('atom2')
    const atom3 = await user1.getOrCreateAtom('atom3')

    const triple = await user1.getCreateOrDepositOnTriple(
      atom1.vaultId,
      atom2.vaultId,
      atom3.vaultId,
      parseEther('1')
    )

    const user2 = await getIntuition(1002)

    const hash = await user2.contract.write.deposit(
      [user2.account.address, triple.vaultId, 2n, 0n],
      { value: parseEther('1') }
    )


    const tripleQuery = graphql(`
      query TripleNumbers($termId: String!) {
        triple(term_id: $termId) {
          term {
            vaults {
              curve_id
              position_count
              total_assets
              total_shares
              current_share_price
              market_cap
            }
          }
          counter_term {
            vaults {
              curve_id
              position_count
              total_assets
              total_shares
              current_share_price
              market_cap
            }
          }
          triple_term {
            total_position_count
            total_assets
            total_market_cap
            total_market_cap
          }
          triple_vault {
            position_count
            market_cap
            total_assets
            total_shares  
            curve_id
          }
        }
        triple_vaults(where:  {
           term_id:  {
              _eq: $termId
           }
        }) {
          curve_id
          term_id
          position_count
          market_cap
          total_assets
          total_shares  
          counter_term_id
        }
      }
    `)

    const result = await execute(
      tripleQuery,
      { termId: triple.vaultId.toString() })

    expect(result).toBeDefined()
    // term
    const positions = result?.triple?.term?.vaults.reduce((sum, vault) => sum + vault.position_count, 0)
    const totalAssets = result?.triple?.term?.vaults.reduce((sum, vault) => sum + BigInt(vault.total_assets), 0n)
    const marketCap = result?.triple?.term?.vaults.reduce((sum, vault) => sum + BigInt(vault.market_cap), 0n)

    // counter term
    const counterPositions = result?.triple?.counter_term?.vaults.reduce((sum, vault) => sum + vault.position_count, 0)
    const counterTotalAssets = result?.triple?.counter_term?.vaults.reduce((sum, vault) => sum + BigInt(vault.total_assets), 0n)
    const counterMarketCap = result?.triple?.counter_term?.vaults.reduce((sum, vault) => sum + BigInt(vault.market_cap), 0n)

    expect(BigInt(result.triple?.triple_term?.total_position_count)).toEqual(BigInt(positions! + counterPositions!))
    expect(BigInt(result.triple?.triple_term?.total_assets)).toEqual(totalAssets! + counterTotalAssets!)
    expect(BigInt(result.triple?.triple_term?.total_market_cap)).toEqual(marketCap! + counterMarketCap!)
  })
})
