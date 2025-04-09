import { expect, test, suite } from 'vitest'
import { getIntuition, pinJson, PredicateType } from './setup/utils.js'

suite('create system predicates', async () => {
  const admin = await getIntuition(0)

  const followAtom = await admin.getOrCreateAtom(
    PredicateType.FollowAction,
  )

  const keywordsAtom = await admin.getOrCreateAtom(
    PredicateType.Keywords,
  )

  const thingAtom = await admin.getOrCreateAtom(
    PredicateType.Thing,
  )

  const organizationPredicate = await admin.getOrCreateAtom(
    PredicateType.Organization,
  )

  const personAtom = await admin.getOrCreateAtom(
    PredicateType.Person,
  )
  expect(followAtom).toBeDefined()
  expect(keywordsAtom).toBeDefined()
  expect(thingAtom).toBeDefined()
  expect(organizationPredicate).toBeDefined()
  expect(personAtom).toBeDefined()

  test('create admin atom and org triple', async () => {

    const adminAtom = await admin.getOrCreateAtom(admin.account.address)

    expect(adminAtom).toBeDefined()

    const uri = await pinJson({
      '@context': 'https://schema.org',
      '@type': 'Organization',
      name: 'Intuition Systems',
      description: 'Intuition Systems',
      image: 'https://avatars.githubusercontent.com/u/94311139?s=200&v=4',
      url: 'https://intuition.systems',
    })

    expect(uri).toBeDefined()

    const intuitionSystems = await admin.getOrCreateAtom(uri)

    expect(intuitionSystems).toBeDefined()

    const { vaultId } = await admin.getCreateOrDepositOnTriple(
      adminAtom.vaultId,
      organizationPredicate.vaultId,
      intuitionSystems.vaultId,
    )

    expect(vaultId).toBeDefined()
  })

})
