import { getIntuition, wait } from './setup/utils.js'

async function main() {
  console.log('Starting atom creation script...\n')

  const intuition = await getIntuition(0)

  const userAddress = '0x25d5C9DbC1E12163B973261A08739927E4F72BA8'

  console.log(`Creating atom for address ${userAddress}...`)
  const atom = await intuition.getOrCreateAtom(userAddress)
  console.log(`Atom vaultId: ${atom.vaultId}`)

  if (atom.hash) {
    console.log('Waiting for transaction to be indexed...')
    await wait(atom.hash)
  }

  console.log('\nAtom creation complete!')
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
