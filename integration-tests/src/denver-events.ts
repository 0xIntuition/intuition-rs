import { getIntuition, getOrCreateAtom, pinJson } from './utils'
import { pinThing } from './graphql'

async function main() {
  const admin = await getIntuition(0)

  const tag = await getOrCreateAtom(
    admin.multivault,
    'https://schema.org/keywords',
  )

  const thing = await getOrCreateAtom(
    admin.multivault,
    'https://schema.org/Thing',
  )

  const adminAccount = await getOrCreateAtom(admin.multivault, admin.account.address)

  const denverEvents = await getOrCreateAtom(admin.multivault, await pinThing({
    name: 'Denver Events',
    description: 'Denver Events',
    image: '',
    url: '',
  }))

  const event = await getOrCreateAtom(admin.multivault, await pinJson({
    "name": "GM Podcast @ EthDenver",
    "description": "",
    "location": "Denver, Colorado",
    "organizers": [
      "Genzio"
    ],
    "date": "Monday, February 24",
    "time": "10:00 AM - Mar 2, 7:00 PM MST",
    "speakers": [
      "Genzio team and special guests"
    ],
    "category": "Crypto",
    "link": "# GM Podcast @ EthDenver",
    "hosted_by": "Genzio"
  }))

  console.log(`event: ${event}`)

  await admin.multivault.createTriple({
    subjectId: event,
    predicateId: tag,
    objectId: denverEvents,
  })

}

main()
  .catch(console.error)
  .finally(() => console.log('done'))
