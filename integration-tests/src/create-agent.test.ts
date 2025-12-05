import { expect, test, suite } from 'vitest'
import { execute, getIntuition, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

/**
 * Integration tests for CAIP-22 NFT atom resolution.
 *
 * These tests create atoms using CAIP-22 URIs that reference real NFTs
 * from the AgentRegistry contract on Base Sepolia. The resolver consumer
 * should automatically fetch the tokenURI from the contract, retrieve
 * the metadata JSON, and populate the atom's label, image, and json_object.
 *
 * CAIP-22 format: caip22:eip155:{chain_id}/erc721:{contract_address}/{token_id}
 *
 * Test NFTs used:
 * - Base Sepolia AgentRegistry (chain 84532): 0x8004AA63c570c570eBF15376c0dB199918BFe9Fb
 * - Token IDs: 1563, 1564 (real agent NFTs from QuestFlow)
 */

// Base Sepolia AgentRegistry contract
const AGENT_REGISTRY_CONTRACT = '0x8004AA63c570c570eBF15376c0dB199918BFe9Fb'
const BASE_SEPOLIA_CHAIN_ID = 84532

// Helper to create CAIP-22 URI
function createCaip22Uri(chainId: number, contractAddress: string, tokenId: number | string): string {
  return `caip22:eip155:${chainId}/erc721:${contractAddress}/${tokenId}`
}

suite('create agent atom (CAIP-22)', async () => {
  const alice = await getIntuition(4)

  // Create a CAIP-22 atom pointing to a real AgentRegistry NFT
  const agentCaip22 = createCaip22Uri(BASE_SEPOLIA_CHAIN_ID, AGENT_REGISTRY_CONTRACT, 1563)
  console.log(`Creating CAIP-22 atom: ${agentCaip22}`)

  const agentAtom = await alice.getOrCreateAtom(agentCaip22)

  expect(agentAtom).toBeDefined()
  expect(agentAtom.vaultId).toBeDefined()

  test('atom is created with CAIP-22 data', async () => {
    await wait(agentAtom.hash)

    // Query the atom to verify it was created
    const result = await execute(
      graphql(`query GetCaip22AtomBasic($termId: String!) {
        atom(term_id: $termId) {
          term_id
          data
          type
          resolving_status
        }
      }`),
      { termId: agentAtom.vaultId }
    )

    expect(result).toBeDefined()
    expect(result.atom).toBeDefined()
    expect(result.atom?.data).toBe(agentCaip22)
    expect(result.atom?.type).toBe('Caip22')
  })

  test('atom is resolved with NFT metadata', async () => {
    // Wait longer for the resolver to fetch tokenURI and metadata
    // The resolver needs to:
    // 1. Make RPC call to Base Sepolia to get tokenURI
    // 2. Fetch metadata JSON from the tokenURI
    // 3. Store as json_object
    // 4. Update atom label and image
    await new Promise(resolve => setTimeout(resolve, 10000))

    const result = await execute(
      graphql(`query GetCaip22AtomResolved($termId: String!) {
        atom(term_id: $termId) {
          term_id
          data
          type
          label
          image
          resolving_status
          value {
            json_object {
              data
            }
          }
        }
      }`),
      { termId: agentAtom.vaultId }
    )

    expect(result).toBeDefined()
    expect(result.atom).toBeDefined()

    // Atom type should be Caip22
    expect(result.atom?.type).toBe('Caip22')

    // Atom should be resolved
    expect(result.atom?.resolving_status).toBe('Resolved')

    // Label should be populated from NFT metadata "name" field
    expect(result.atom?.label).toBeDefined()
    expect(result.atom?.label).not.toBe('')
    console.log(`Resolved agent name: ${result.atom?.label}`)

    // Image should be populated from NFT metadata "image" field
    if (result.atom?.image) {
      console.log(`Resolved agent image: ${result.atom?.image}`)
    }

    // JSON object should contain the full NFT metadata
    expect(result.atom?.value?.json_object).toBeDefined()
    if (result.atom?.value?.json_object?.data) {
      // Note: json_object.data is a jsonb field, which Hasura returns as a parsed object, not a string
      const metadata = result.atom.value.json_object.data as Record<string, unknown>
      console.log(`Agent metadata:`, metadata)

      // Verify expected EIP-8004 AgentRegistry metadata fields
      expect(metadata.name).toBeDefined()
      expect(metadata.description).toBeDefined()
      expect(metadata.endpoints).toBeDefined()
    }
  })
})

suite('create multiple agent atoms', async () => {
  const bob = await getIntuition(5)

  // Create two different agent atoms
  const agent1Caip22 = createCaip22Uri(BASE_SEPOLIA_CHAIN_ID, AGENT_REGISTRY_CONTRACT, 1563)
  const agent2Caip22 = createCaip22Uri(BASE_SEPOLIA_CHAIN_ID, AGENT_REGISTRY_CONTRACT, 1564)

  test('create first agent atom', async () => {
    const agent1 = await bob.getOrCreateAtom(agent1Caip22)
    expect(agent1.vaultId).toBeDefined()
    await wait(agent1.hash)

    const result = await execute(
      graphql(`query GetFirstAgentAtom($termId: String!) {
        atom(term_id: $termId) {
          data
          type
        }
      }`),
      { termId: agent1.vaultId }
    )

    expect(result.atom?.type).toBe('Caip22')
  })

  test('create second agent atom', async () => {
    const agent2 = await bob.getOrCreateAtom(agent2Caip22)
    expect(agent2.vaultId).toBeDefined()
    await wait(agent2.hash)

    const result = await execute(
      graphql(`query GetSecondAgentAtom($termId: String!) {
        atom(term_id: $termId) {
          data
          type
        }
      }`),
      { termId: agent2.vaultId }
    )

    expect(result.atom?.type).toBe('Caip22')
  })

  test('query all Caip22 atoms', async () => {
    // Wait for resolution
    await new Promise(resolve => setTimeout(resolve, 10000))

    const result = await execute(graphql(`
      query GetCaip22Atoms {
        atoms(where: { type: { _eq: "Caip22" } }) {
          term_id
          data
          label
          image
          resolving_status
        }
      }
    `))

    expect(result).toBeDefined()
    expect(result.atoms).toBeDefined()
    expect(result.atoms.length).toBeGreaterThanOrEqual(2)

    console.log(`Found ${result.atoms.length} CAIP-22 atoms:`)
    for (const atom of result.atoms) {
      console.log(`  - ${atom.label || 'Pending'}: ${atom.data} (${atom.resolving_status})`)
    }
  })
})

suite('create agent triple (subject is agent)', async () => {
  const charlie = await getIntuition(6)

  // Create atoms for the triple
  const agentCaip22 = createCaip22Uri(BASE_SEPOLIA_CHAIN_ID, AGENT_REGISTRY_CONTRACT, 1563)
  const agentAtom = await charlie.getOrCreateAtom(agentCaip22)

  const typeAtom = await charlie.getOrCreateAtom('type')
  const aiAgentAtom = await charlie.getOrCreateAtom('AI Agent')

  await wait(agentAtom.hash)
  await wait(typeAtom.hash)
  await wait(aiAgentAtom.hash)

  test('create triple: agent -> type -> AI Agent', async () => {
    const triple = await charlie.getCreateOrDepositOnTriple(
      agentAtom.vaultId,
      typeAtom.vaultId,
      aiAgentAtom.vaultId,
    )

    expect(triple.vaultId).toBeDefined()
    await wait(triple.hash)

    // Query the triple
    const result = await execute(graphql(`
      query GetAgentTriple($tripleId: String!) {
        triple(term_id: $tripleId) {
          term_id
          subject {
            data
            type
            label
          }
          predicate {
            data
          }
          object {
            data
          }
        }
      }
    `), { tripleId: triple.vaultId })

    expect(result).toBeDefined()
    expect(result.triple).toBeDefined()
    expect(result.triple?.subject?.type).toBe('Caip22')
    expect(result.triple?.predicate?.data).toBe('type')
    expect(result.triple?.object?.data).toBe('AI Agent')

    console.log(`Created triple: ${result.triple?.subject?.label || result.triple?.subject?.data} -> type -> AI Agent`)
  })
})

suite('deposit on existing CAIP-22 atom triggers re-resolution', async () => {
  const dave = await getIntuition(7)

  // Create a CAIP-22 atom
  const agentCaip22 = createCaip22Uri(BASE_SEPOLIA_CHAIN_ID, AGENT_REGISTRY_CONTRACT, 1563)
  const agentAtom = await dave.getOrCreateAtom(agentCaip22)
  await wait(agentAtom.hash)

  test('initial atom is created and resolved', async () => {
    // Wait for initial resolution
    await new Promise(resolve => setTimeout(resolve, 10000))

    const result = await execute(
      graphql(`query GetCaip22AtomInitial($termId: String!) {
        atom(term_id: $termId) {
          type
          resolving_status
          label
        }
      }`),
      { termId: agentAtom.vaultId }
    )

    expect(result.atom?.type).toBe('Caip22')
    expect(result.atom?.resolving_status).toBe('Resolved')
    console.log(`Initial resolution complete: ${result.atom?.label}`)
  })

  test('deposit triggers re-resolution', async () => {
    // Get another user to deposit on the same atom
    const eve = await getIntuition(8)

    // Create/deposit on the same CAIP-22 atom
    const depositResult = await eve.getOrCreateAtom(agentCaip22)

    // Since atom already exists, this should deposit on it
    // The deposited event should trigger re-resolution

    // Wait for re-resolution
    await new Promise(resolve => setTimeout(resolve, 10000))

    const result = await execute(
      graphql(`query GetCaip22AtomAfterDeposit($termId: String!) {
        atom(term_id: $termId) {
          type
          resolving_status
          label
        }
      }`),
      { termId: depositResult.vaultId }
    )

    // Atom should still be resolved after re-resolution
    expect(result.atom?.resolving_status).toBe('Resolved')
    console.log(`After deposit, atom status: ${result.atom?.resolving_status}`)
  })
})
