import { expect, test } from 'vitest'
import { getIntuition, PredicateType } from './setup/utils'
import { pinPerson } from './graphql'

test('create person triple', async () => {
  const alice = await getIntuition(1)

  const personPredicateId = await alice.getOrCreateAtom(
    PredicateType.Person,
  )

  const aliceAtomId = await alice.getOrCreateAtom(
    alice.account.address
  )

  const uri = await pinPerson({
    identifier: alice.account.address,
    name: 'Alice',
    description: 'Intern at Intuition Systems',
    email: 'alice@intuition.systems',
    image: 'https://avatars.githubusercontent.com/u/94311139?s=200&v=4',
    url: 'https://intuition.systems',
  })

  const alicePersonId = await alice.getOrCreateAtom(uri)

  const vaultId = await alice.getCreateOrDepositOnTriple(
    aliceAtomId,
    personPredicateId,
    alicePersonId,
  )

  expect(personPredicateId).toBeDefined()
  expect(aliceAtomId).toBeDefined()
  expect(uri).toBeDefined()
  expect(alicePersonId).toBeDefined()
  expect(vaultId).toBeDefined()

})
