import { createServerClient, configureClient } from '@0xintuition/graphql'

configureClient({
  apiUrl: 'http://localhost:8080/v1/graphql',
})
export const graphqlClient = createServerClient({})

export async function getVaultPositions(vaultId: string) {
  const result: any = await graphqlClient.request(
    `
      query VaultPositions($vaultId: String!) {
        vaultPositions(vaultId: $vaultId) {
          vaultId
          shares
        }
      }
    `,
    { vaultId },
  )
  return result.vaultPositions
}

export async function pinThing(thing: { name: string, description: string, image: string, url: string }) {
  const result: any = await graphqlClient.request(`
mutation PinThing($thing: PinThingInput!) {
  pinThing(thing: $thing) {
    uri
  }
}`, { thing })
  console.log(`pinned thing: ${result.pinThing.uri}`)
  return result.pinThing.uri as string
}

export async function pinPerson(person: { name: string, description: string, image: string, url: string, email: string, identifier: string }) {
  const result: any = await graphqlClient.request(`
mutation PinPerson($person: PinPersonInput!) {
  pinPerson(person: $person) {
    uri
  }
}`, { person })
  console.log(`pinned person: ${result.pinPerson.uri}`)
  return result.pinPerson.uri as string
}

