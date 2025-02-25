import { getIntuition, getOrCreateAtom } from './utils'

async function main() {
  const admin = await getIntuition(0)

  const foo = await getOrCreateAtom(
    admin.multivault,
    'foo',
  )

  const bar = await getOrCreateAtom(
    admin.multivault,
    'bar',
  )

  const baz = await getOrCreateAtom(
    admin.multivault,
    'baz',
  )

  const config = await admin.multivault.getGeneralConfig()

  const { vaultId } = await admin.multivault.createTriple({
    subjectId: foo,
    predicateId: bar,
    objectId: baz,
    initialDeposit: config.minDeposit,
  })
  console.log(`Created triple: ${vaultId}`)

  // check the position
  const { shares, totalUserAssets } = await admin.multivault.getVaultStateForUser(vaultId, admin.account.address)
  console.log(`Shares: ${shares}`)
  console.log(`Total user assets: ${totalUserAssets}`)

}

main()
  .catch(console.error)
  .finally(() => console.log('done'))
