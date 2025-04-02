import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

suite('follow account', async () => {
  const admin = await getIntuition(0)
  const alice = await getIntuition(1)
  const bob = await getIntuition(2)

  const person = await admin.getOrCreateAtom(
    SystemAtom.Person,
  )

  const followAction = await admin.getOrCreateAtom(
    SystemAtom.FollowAction,
  )

  const bobAtom = await bob.getOrCreateAtom(
    bob.account.address
  )

  const triple = await alice.getCreateOrDepositOnTriple(
    person.vaultId,
    followAction.vaultId,
    bobAtom.vaultId,
  )

  expect(triple.vaultId).toBeDefined()

  test('query following', async () => {
    await wait(triple.hash)
    const result = await execute(
      graphql(`query Following($address: String!) {
        following(args: {address: $address}) {
          id
          atom_id
        }
      }
      `),
      { address: alice.account.address.toLowerCase() }
    )
    expect(result).toBeDefined()
    expect(result.following.length).toBe(1)
    expect(result.following[0].id).toBe(bobAtom.vaultId.toString())
  })

})
