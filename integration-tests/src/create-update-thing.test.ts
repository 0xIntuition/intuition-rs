import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, PredicateType, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

suite('create person triple', async () => {
  const alice = await getIntuition(1)

  const thingPredicate = await alice.getOrCreateAtom(
    PredicateType.Thing,
  )

  const originalThing = await alice.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Foo',
    description: 'Lorem ipsum',
    image: 'https://example.com/cat.png',
    url: 'https://example.com',
  }))

  const updatedThing = await alice.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Example Domain',
    description: 'This domain is for use in illustrative examples in documents',
  }))

  const generalConfig = await alice.multivault.getGeneralConfig()

  const triple = await alice.getCreateOrDepositOnTriple(
    originalThing.vaultId,
    thingPredicate.vaultId,
    updatedThing.vaultId,
    generalConfig.minDeposit,
  )

  expect(thingPredicate).toBeDefined()
  expect(originalThing).toBeDefined()
  expect(updatedThing).toBeDefined()
  expect(triple.vaultId).toBeDefined()

  test('query thing with claims', async () => {
    await wait(triple.hash)
    const result = await execute(
      graphql(`query AtomWithClaims($atomId: numeric!, $address: String) {
        atom(id: $atomId) {
          id
          label
          value {
            thing {
              name
              description
              url
              image
            }
          }
        }
        claims(
          where: { account_id: { _eq: $address }, subject_id: { _eq: $atomId } }
          order_by: [{ shares: desc }]
        ) {
          predicate {
            id
            type
            label
          }
          object {
            value {
              thing {
                name
                description
                url
                image
              }
            }
          }
        }
        claims_from_following(
          args: { address: $address }
          where: { subject_id: { _eq: $atomId } }
        ) {
          predicate {
            id
            type
            label
          }
          object {
            value {
              thing {
                name
                description
                url
                image
              }
            }
          }
        }
      }`),
      { atomId: originalThing.vaultId.toString(), address: alice.account.address.toLowerCase() })
    expect(result).toBeDefined()
    expect(result.atom.label).toBe('Foo')
    expect(result.claims.length).toBe(1)
    expect(result.claims[0].predicate.label).toBe('is thing')
    expect(result.claims[0].object.value.thing.name).toBe('Example Domain')
  })

})
