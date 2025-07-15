import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

suite('create ai agents', () => {
  test('should create ai agent', async () => {
    const admin = await getIntuition(1)

    const type = await admin.getOrCreateAtom('type')

    const agent = await admin.getOrCreateAtom('agent')

    const url = await admin.getOrCreateAtom('url')

    const capabilities = await admin.getOrCreateAtom('capabilities')

    const product_recommendation = await admin.getOrCreateAtom('product_recommendation')

    const defi = await admin.getOrCreateAtom('defi')
    await wait(defi.hash)

    const user = await getIntuition(11)
    const userAtom = await user.getOrCreateAtom(user.account.address)

    const url_example = await user.getOrCreateAtom('https://example.com/a2a/v1')
    await wait(url_example.hash)

    await user.getCreateOrDepositOnTriple(
      userAtom.vaultId,
      type.vaultId,
      agent.vaultId,
    )

    await user.getCreateOrDepositOnTriple(
      userAtom.vaultId,
      url.vaultId,
      url_example.vaultId,
    )

    await user.getCreateOrDepositOnTriple(
      userAtom.vaultId,
      capabilities.vaultId,
      defi.vaultId,
    )

    const triple = await user.getCreateOrDepositOnTriple(
      userAtom.vaultId,
      capabilities.vaultId,
      product_recommendation.vaultId,
    )
    await wait(triple.hash)

    expect(triple.hash).toBeDefined()
  })

  test('query ai agent triples', async () => {
    const result = await execute(graphql(`
      query GetAgents {
        triples(where: {
          _and: [
            { predicate: { data: { _eq: "type" } } },
            { object: { data: { _eq: "agent" } } }
          ]
        }) {
          subject {
            data
            claims: as_subject_triples {
              predicate {
                data
              }
              object {
                data
              }
            }
          }
        }
      }
      `))
    expect(result).toBeDefined()
  })

  test('query ai agents using account positions', async () => {
    const user = await getIntuition(11)

    const result = await execute(graphql(`
      query GetAgentsForAccount($address: String) {
        positions(where:{
          _and: [
            {account_id: {_eq: $address}}
            {shares: {_gt: 0}}
            {term:{triple: { predicate: {data: {_eq: "type"}}}}}      
            {term:{triple: { object: {data: {_eq: "agent"}}}}}      
          ]
        }) {
          account_id
          term {
            triple {
              subject {
                  data
                  claims: as_subject_triples(where:{
                    _and: [
                      {positions: {account_id: {_eq: $address}}}
                      {positions: {shares: {_gt: 0}}}
                    ]
                  }) {
                    predicate {
                      data
                    }
                    object {
                      data
                    }
                  }
                }
            }
          }
        }
      }
            `), { address: user.account.address })
    expect(result.positions.length).toBeGreaterThan(0)
  })

})
