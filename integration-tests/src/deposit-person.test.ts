import { expect, test } from 'vitest'
import { getIntuition, pinJson } from './setup/utils.js'
import { parseEther } from 'viem'

test('deposit on person atom', async () => {
  const alice = await getIntuition(3)

  const uri = await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Person',
    name: 'Alice',
    description: 'Intern at Intuition Systems',
    email: 'alice@intuition.systems',
    image: 'https://avatars.githubusercontent.com/u/94311139?s=200&v=4',
    url: 'https://intuition.systems',
  })

  const alicePerson = await alice.getOrCreateAtom(uri)

  const hash = await alice.contract.write.deposit([alice.account.address, alicePerson.vaultId, 1n, 0n], {
    value: parseEther('0.5')
  })

  console.log('Deposit tx:', hash)

  expect(hash).toBeDefined()

})
