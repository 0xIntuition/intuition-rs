import { expect, test, suite } from 'vitest'
import { execute, getIntuition, oxToBackslashX, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'
import { parseEther } from 'viem'

suite('follow account and semantic search', async () => {
  const admin = await getIntuition(0)
  const alice = await getIntuition(1)
  const bob = await getIntuition(2)
  const carol = await getIntuition(3)

  const thing = await admin.getOrCreateAtom(
    SystemAtom.Thing,
  )

  const followAction = await admin.getOrCreateAtom(
    SystemAtom.FollowAction,
  )

  const bobAtom = await bob.getOrCreateAtom(
    bob.account.address
  )

  // Alice follows Bob
  const triple = await alice.getCreateOrDepositOnTriple(
    thing.vaultId,
    followAction.vaultId,
    bobAtom.vaultId,
  )

  expect(triple.vaultId).toBeDefined()

  // Bob creates a cat and a table

  const cat = await bob.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Cat',
    description: 'A cat is a small domesticated carnivorous mammal with soft fur, a short snout, and retractile claws. It is widely kept as a pet or for catching mice, and many breeds have been developed.'
  }))

  await wait(cat.hash)
  await bob.contract.write.deposit(
    [bob.account.address, cat.vaultId, 1n, 0n],
    { value: parseEther('0.1') }
  )

  const table = await bob.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Table',
    description: 'A table is a piece of furniture with a flat top and one or more legs, used for placing objects or eating.',
  }))

  // Carol create beagle
  const beagle = await carol.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Beagle',
    description: 'A beagle is a small breed of dog.',
  }))


  // Wait for 10 seconds
  console.log('Waiting for 10 seconds... to make sure term embeddings are updated')
  await new Promise(resolve => setTimeout(resolve, 10000))


  test('semantic search for table', async () => {
    await wait(table.hash)
    const result = await execute(
      graphql(`query SearchTerm($query: String!) {
        search_term(args: {query: $query}, limit: 2) {
          id
          atom {
            label
          }
        }
      }
      `),
      { query: 'something that you put your coffe on' }
    )
    expect(result).toBeDefined()
    expect(result.search_term.length).toBe(2)
    expect(result.search_term[0].id).toBe(oxToBackslashX(table.vaultId))
  })
  test('semantic search for cat', async () => {
    await wait(cat.hash)
    const result = await execute(
      graphql(`query SearchTerm($query: String!) {
        search_term(args: {query: $query}, limit: 2) {
          id
          atom {
            label
          }
        }
      }
      `),
      { query: 'a small domesticated carnivorous mammal' }
    )
    expect(result).toBeDefined()
    expect(result.search_term[0].id).toBe(oxToBackslashX(cat.vaultId))
  })

  test('semantic search for dog', async () => {
    await wait(beagle.hash)
    const result = await execute(
      graphql(`query SearchTerm($query: String!) {
        search_term(args: {query: $query}, limit: 2) {
          id
          atom {
            label
          }
        }
      }
      `),
      { query: 'собака' }
    )
    expect(result).toBeDefined()
    expect(result.search_term.length).toBe(2)
    expect(result.search_term[0].id).toBe(oxToBackslashX(beagle.vaultId))
  })

  test('semantic search from following', async () => {
    await wait(beagle.hash)
    const result = await execute(
      graphql(`query SearchFromFollowing($address: String!, $query: String!) {
        search_term_from_following(args: {address: $address, query: $query} limit: 2) {
          id
          atom {
            label
          }
        }
      }
      `),
      { address: alice.account.address, query: 'domestic animal' }
    )
    expect(result).toBeDefined()
    expect(result.search_term_from_following.length).toBe(2)
    expect(result.search_term_from_following[0].id).toBe(oxToBackslashX(cat.vaultId))
  })
})
