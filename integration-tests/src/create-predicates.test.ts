import { expect, test, suite } from 'vitest'
import { getIntuition, pinJson, SystemAtom } from './setup/utils.js'

suite('create system predicates', async () => {
  const admin = await getIntuition(0)

  const followAction = await admin.getOrCreateAtom(
    SystemAtom.FollowAction,
  )

  const hasTag = await admin.getOrCreateAtom(
    SystemAtom.Keywords,
  )

  const thing = await admin.getOrCreateAtom(
    SystemAtom.Thing,
  )

  const organization = await admin.getOrCreateAtom(
    SystemAtom.Organization,
  )

  const personAtom = await admin.getOrCreateAtom(
    SystemAtom.Person,
  )
  expect(followAction).toBeDefined()
  expect(hasTag).toBeDefined()
  expect(thing).toBeDefined()
  expect(organization).toBeDefined()
  expect(personAtom).toBeDefined()

  test('create admin atom and org triple', async () => {

    const adminAccount = await admin.getOrCreateAtom(admin.account.address)

    expect(adminAccount).toBeDefined()

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
      adminAccount.vaultId,
      organization.vaultId,
      intuitionSystems.vaultId,
    )

    expect(vaultId).toBeDefined()
  })

})
