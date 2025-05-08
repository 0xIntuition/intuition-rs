import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

suite('create person triple', async () => {
  const admin = await getIntuition(1)

  // System atoms

  const person = await admin.getOrCreateAtom(
    SystemAtom.Person,
  )

  const skills = await admin.getOrCreateAtom(
    SystemAtom.Skills,
  )

  const memberOf = await admin.getOrCreateAtom(
    SystemAtom.MemberOf,
  )

  const wasAssociatedWith = await admin.getOrCreateAtom(
    SystemAtom.WasAssociatedWith,
  )

  // People

  // Maya

  const maya = await getIntuition(11)

  const mayaAccount = await maya.getOrCreateAtom(
    maya.account.address
  )

  const mayaPerson = await maya.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Person',
    name: 'Maya',
    description: 'Product manager passionate about building great products',
    email: 'maya@nova-biotech.example',
    url: 'https://nova-biotech.example',
  }))

  const triple = await maya.getCreateOrDepositOnTriple(
    mayaAccount.vaultId,
    person.vaultId,
    mayaPerson.vaultId,
  )

  expect(person).toBeDefined()
  expect(mayaAccount).toBeDefined()
  expect(mayaPerson).toBeDefined()
  expect(triple.vaultId).toBeDefined()
  await wait(triple.hash)

})
