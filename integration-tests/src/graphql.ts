import { graphql } from './graphql/gql.js';
import { execute } from './setup/utils.js';

const pinThingQuery = graphql(`mutation PinThing($thing: PinThingInput!) {
  pinThing(thing: $thing) {
    uri
  }
}`)

export async function pinThing(thing: { name: string, description: string, image: string, url: string }) {
  const result = await execute(pinThingQuery, { thing })
  console.log(`pinned thing: ${result.pinThing.uri}`)
  return result.pinThing.uri as string
}

export async function pinPerson(person: { name: string, description: string, image: string, url: string, email: string, identifier: string }) {
  const result = await execute(graphql(`mutation PinPerson($person: PinPersonInput!) {
  pinPerson(person: $person) {
    uri
  }
}`), { person })
  console.log(`pinned person: ${result.pinPerson.uri}`)
  return result.pinPerson.uri as string
}

