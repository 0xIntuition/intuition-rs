import { getIntuition, wait } from './setup/utils.js'
import { parseEther, toHex } from 'viem'

async function main() {
  console.log('Starting deposit on atom script...\n')

  const intuition = await getIntuition(0)

  const userAddress = '0x25d5C9DbC1E12163B973261A08739927E4F72BA8'
  const depositAmount = parseEther('0.1')

  // Resolve the atom's vaultId from the address
  const vaultId = await intuition.contract.read.calculateAtomId([toHex(userAddress)])
  console.log(`Atom vaultId for ${userAddress}: ${vaultId}`)

  // Deposit on the atom
  console.log(`Depositing ${depositAmount} wei on atom...`)
  const hash = await intuition.contract.write.deposit(
    [intuition.account.address, vaultId, 1n, 0n],
    { value: depositAmount },
  )
  console.log(`Transaction hash: ${hash}`)

  console.log('Waiting for transaction to be indexed...')
  await wait(hash)

  console.log('\nDeposit complete!')
}

main()
  .then(() => {
    console.log('\nScript completed successfully')
    process.exit(0)
  })
  .catch((error) => {
    console.error('\nError:', error)
    process.exit(1)
  })
