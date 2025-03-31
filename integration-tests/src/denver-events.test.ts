import { expect, test, suite } from 'vitest'
import { getIntuition, pinJson, PredicateType } from './setup/utils.js'
import { pinThing } from './graphql.js'

test('create json_object with a tag', async () => {
  const bob = await getIntuition(2)

  const hasTag = await bob.getOrCreateAtom(
    PredicateType.Keywords,
  )

  const denverEvents = await bob.getOrCreateAtom(await pinThing({
    name: 'Denver Events',
    description: 'Denver Events',
    image: '',
    url: '',
  }))

  const event = await bob.getOrCreateAtom(await pinJson({
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

  const triple = await bob.getCreateOrDepositOnTriple(
    event,
    hasTag,
    denverEvents,
  )

  expect(hasTag).toBeDefined()
  expect(denverEvents).toBeDefined()
  expect(event).toBeDefined()
  expect(triple).toBeDefined()

})

