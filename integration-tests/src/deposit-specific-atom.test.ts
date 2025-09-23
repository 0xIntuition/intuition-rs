import { expect, test } from 'vitest'
import { getIntuition } from './setup/utils.js'
import { parseEther, toHex } from 'viem'

test('deposit on atom with specific raw data', async () => {
  const alice = await getIntuition(3)

  // The specific term ID you provided
  const vaultId = '0x937959d6af46f1f6bb0b7834c13947cb3ace6628b334272efaa8fdfbf8df2016'
  
  console.log('Using vault ID:', vaultId)

  // Check if the atom exists
  let atomData = '0x'
  try {
    atomData = await alice.contract.read.getAtom([vaultId])
    console.log('Atom data:', atomData)
  } catch (error) {
    console.log('Atom does not exist yet')
  }

  // Deposit on the atom
  const hash = await alice.contract.write.deposit([alice.account.address, vaultId, 1n, 0n], {
    value: parseEther('0.5')
  })

  console.log('Deposit tx:', hash)

  expect(hash).toBeDefined()
  expect(vaultId).toBeDefined()
})
