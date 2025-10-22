import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

suite('tmp', () => {
  test('tmp', async () => {
    const admin = await getIntuition(1)

    const person = await admin.getOrCreateAtom(SystemAtom.Person)
    await wait(person.hash)

    const maya = await getIntuition(2)
    const config = await maya.contract.read.getGeneralConfig()
    console.log(config)

    const depositHash = await maya.contract.write.deposit(
      [maya.account.address, person.vaultId, 1n, 0n],
      {
        value: config.minDeposit
      })
    await wait(depositHash)
    expect(person).toBeDefined()
  })
})
