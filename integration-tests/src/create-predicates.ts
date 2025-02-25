import { getIntuition, getOrCreateAtom } from './utils'

async function main() {
  const admin = await getIntuition(0)

  await getOrCreateAtom(
    admin.multivault,
    'https://schema.org/FollowAction',
  )
  await getOrCreateAtom(
    admin.multivault,
    'https://schema.org/keywords',
  )
  await getOrCreateAtom(
    admin.multivault,
    'https://schema.org/Thing',
  )
  const organizationPredicate = await getOrCreateAtom(
    admin.multivault,
    'https://schema.org/Organization',
  )
  await getOrCreateAtom(
    admin.multivault,
    'https://schema.org/Person',
  )

  // const adminAccount = await getOrCreateAtom(admin.multivault, admin.account.address)

  // const uri = await pinThing({
  //   name: 'Intuition Systems',
  //   description: 'Intuition Systems',
  //   image: 'https://avatars.githubusercontent.com/u/94311139?s=200&v=4',
  //   url: 'https://intuition.systems',
  // })

  // const intuitionSystems = await getOrCreateAtom(admin.multivault, uri)

  // await admin.multivault.createTriple({
  //   subjectId: adminAccount,
  //   predicateId: organizationPredicate,
  //   objectId: intuitionSystems,
  // })

}

main()
  .catch(console.error)
  .finally(() => console.log('done'))
