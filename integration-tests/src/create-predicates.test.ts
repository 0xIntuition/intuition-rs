import { expect, test, suite } from 'vitest'
import { getIntuition, PredicateType } from './setup/utils.js'
import { pinThing } from './graphql.js'

suite('create system predicates', async () => {
  const admin = await getIntuition(0)

  const followAtomId = await admin.getOrCreateAtom(
    PredicateType.FollowAction,
  )

  const keywordsAtomId = await admin.getOrCreateAtom(
    PredicateType.Keywords,
  )

  const thingAtomId = await admin.getOrCreateAtom(
    PredicateType.Thing,
  )

  const organizationPredicate = await admin.getOrCreateAtom(
    PredicateType.Organization,
  )

  const personAtomId = await admin.getOrCreateAtom(
    PredicateType.Person,
  )
  expect(followAtomId).toBeDefined()
  expect(keywordsAtomId).toBeDefined()
  expect(thingAtomId).toBeDefined()
  expect(organizationPredicate).toBeDefined()
  expect(personAtomId).toBeDefined()

  test('create admin atom and org triple', async () => {

    const adminAtomId = await admin.getOrCreateAtom(admin.account.address)

    expect(adminAtomId).toBeDefined()

    const uri = await pinThing({
      name: 'Intuition Systems',
      description: 'Intuition Systems',
      image: 'https://avatars.githubusercontent.com/u/94311139?s=200&v=4',
      url: 'https://intuition.systems',
    })

    expect(uri).toBeDefined()

    const intuitionSystems = await admin.getOrCreateAtom(uri)

    expect(intuitionSystems).toBeDefined()

    const vaultId = await admin.getCreateOrDepositOnTriple(
      adminAtomId,
      organizationPredicate,
      intuitionSystems,
    )

    expect(vaultId).toBeDefined()
  })

})
