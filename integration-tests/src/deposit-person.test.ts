import { expect, test } from 'vitest'
import { getIntuition } from './setup/utils.js'
import { parseEther } from 'viem'

test('deposit on person atom', async () => {
  const alice = await getIntuition(3)

  const aliceAccount = await alice.getOrCreateAtom(
    alice.account.address
  )

  const hash = await alice.contract.write.deposit([alice.account.address, aliceAccount.vaultId, 1n, 0n], {
    value: parseEther('0.5')
  })

  console.log('Deposit tx:', hash)

  expect(hash).toBeDefined()

})
