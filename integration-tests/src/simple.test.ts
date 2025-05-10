import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

suite('create person triple', () => {
  test('should create the person triple with all dependencies', async () => {
    const admin = await getIntuition(1)

    const person = await admin.getOrCreateAtom(SystemAtom.Person)
    await wait(person.hash)

    const skills = await admin.getOrCreateAtom(SystemAtom.Skills)
    await wait(skills.hash)

    const memberOf = await admin.getOrCreateAtom(SystemAtom.MemberOf)
    await wait(memberOf.hash)

    const wasAssociatedWith = await admin.getOrCreateAtom(SystemAtom.WasAssociatedWith)
    await wait(wasAssociatedWith.hash)

    const maya = await getIntuition(11)
    const mayaAccount = await maya.getOrCreateAtom(maya.account.address)
    await wait(mayaAccount.hash)

    const mayaPerson = await maya.getOrCreateAtom(await pinJson({
      '@context': 'https://schema.org',
      '@type': 'Person',
      name: 'Maya',
      description: 'Product manager passionate about building great products',
      email: 'maya@nova-biotech.example',
      url: 'https://nova-biotech.example',
    }))
    await wait(mayaPerson.hash)

    const triple = await maya.getCreateOrDepositOnTriple(
      mayaAccount.vaultId,
      person.vaultId,
      mayaPerson.vaultId,
    )
    await wait(triple.hash)

    expect(person).toBeDefined()
    expect(mayaAccount).toBeDefined()
    expect(mayaPerson).toBeDefined()
    expect(triple.vaultId).toBeDefined()
  })
})