import { expect, test } from 'vitest'
import { getIntuition, getOrCreateAtom } from './setup/utils'
import { pinThing } from './graphql'

test('create predicates', async () => {
  const admin = await getIntuition(0)

  const followAtomId = await getOrCreateAtom(
    admin.multivault,
    'https://schema.org/FollowAction',
  )
  expect(followAtomId).toBeDefined()

  const keywordsAtomId = await getOrCreateAtom(
    admin.multivault,
    'https://schema.org/keywords',
  )
  expect(keywordsAtomId).toBeDefined()

  const thingAtomId = await getOrCreateAtom(
    admin.multivault,
    'https://schema.org/Thing',
  )
  expect(thingAtomId).toBeDefined()

  const organizationPredicate = await getOrCreateAtom(
    admin.multivault,
    'https://schema.org/Organization',
  )
  expect(organizationPredicate).toBeDefined()

  const personAtomId = await getOrCreateAtom(
    admin.multivault,
    'https://schema.org/Person',
  )
  expect(personAtomId).toBeDefined()

  const adminAccount = await getOrCreateAtom(admin.multivault, admin.account.address)
  expect(adminAccount).toBeDefined()

  const uri = await pinThing({
    name: 'Intuition Systems',
    description: 'Intuition Systems',
    image: 'https://avatars.githubusercontent.com/u/94311139?s=200&v=4',
    url: 'https://intuition.systems',
  })
  expect(uri).toBeDefined()

  const intuitionSystems = await getOrCreateAtom(admin.multivault, uri)
  expect(intuitionSystems).toBeDefined()

  const { vaultId } = await admin.multivault.createTriple({
    subjectId: adminAccount,
    predicateId: organizationPredicate,
    objectId: intuitionSystems,
  })

  expect(vaultId).toBeDefined()

})
