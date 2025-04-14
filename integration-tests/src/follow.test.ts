import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

suite('follow account', async () => {
  const admin = await getIntuition(0)
  const alice = await getIntuition(1)
  const bob = await getIntuition(2)

  const thing = await admin.getOrCreateAtom(
    SystemAtom.Thing,
  )

  const followAction = await admin.getOrCreateAtom(
    SystemAtom.FollowAction,
  )

  const bobAtom = await bob.getOrCreateAtom(
    bob.account.address
  )

  const triple = await alice.getCreateOrDepositOnTriple(
    thing.vaultId,
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
    expect(result.following[0].atom_id).toBe(bobAtom.vaultId.toString())
    expect(result.following[0].id).toBe(bob.account.address.toLowerCase())
  })

})
