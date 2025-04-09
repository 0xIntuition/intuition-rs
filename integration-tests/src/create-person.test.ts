import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, PredicateType, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

suite('create person triple', async () => {
  const alice = await getIntuition(1)

  const personPredicate = await alice.getOrCreateAtom(
    PredicateType.Person,
  )

  const aliceAtom = await alice.getOrCreateAtom(
    alice.account.address
  )

  const uri = await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Person',
    name: 'Alice',
    description: 'Intern at Intuition Systems',
    email: 'alice@intuition.systems',
    image: 'https://avatars.githubusercontent.com/u/94311139?s=200&v=4',
    url: 'https://intuition.systems',
  })

  const alicePerson = await alice.getOrCreateAtom(uri)

  const triple = await alice.getCreateOrDepositOnTriple(
    aliceAtom.vaultId,
    personPredicate.vaultId,
    alicePerson.vaultId,
  )

  expect(personPredicate).toBeDefined()
  expect(aliceAtom).toBeDefined()
  expect(uri).toBeDefined()
  expect(alicePerson).toBeDefined()
  expect(triple.vaultId).toBeDefined()

  test('query person', async () => {
    await wait(triple.hash)
    const result = await execute(
      graphql(`query Atom($atomId: numeric!) {
        atom(id: $atomId) {
          label
        }
      }`),
      { atomId: alicePerson.vaultId.toString() })
    expect(result).toBeDefined()
    expect(result.atom.label).toBe('Alice')
  })

})
