import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

/**
 * Raw GraphQL executor for queries that use new relationships
 * not yet in codegen (subject_term, predicate_term, object_term).
 */
async function executeRaw<T = any>(query: string, variables?: Record<string, any>): Promise<T> {
  const response = await fetch('http://localhost:8080/v1/graphql', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/graphql-response+json',
    },
    body: JSON.stringify({ query, variables }),
  })
  if (!response.ok) {
    throw new Error(`GraphQL request failed: ${response.statusText}`)
  }
  const json = await response.json() as any
  if (json.errors) {
    throw new Error(`GraphQL errors: ${JSON.stringify(json.errors)}`)
  }
  return json.data as T
}

suite('nested triples', async () => {
  // ============================================================
  // Setup: create atoms and base triples
  // ============================================================
  const alice = await getIntuition(20)
  const bob = await getIntuition(21)

  // Create atom URIs with labels we can assert against
  const aliceAtom = await alice.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Alice',
    description: 'Test person Alice for nested triples',
  }))
  await wait(aliceAtom.hash)

  const bobAtom = await alice.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Bob',
    description: 'Test person Bob for nested triples',
  }))
  await wait(bobAtom.hash)

  const carolAtom = await alice.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Carol',
    description: 'Test person Carol for nested triples',
  }))
  await wait(carolAtom.hash)

  const daveAtom = await alice.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Dave',
    description: 'Test person Dave for nested triples',
  }))
  await wait(daveAtom.hash)

  // Create predicate atoms
  const likesAtom = await alice.getOrCreateAtom('likes')
  await wait(likesAtom.hash)

  const endorsesAtom = await alice.getOrCreateAtom('endorses')
  await wait(endorsesAtom.hash)

  const followsAtom = await alice.getOrCreateAtom('follows')
  await wait(followsAtom.hash)

  // ============================================================
  // Create base triple: (Alice likes Bob)
  // ============================================================
  const tripleAliceLikesBob = await alice.getCreateOrDepositOnTriple(
    aliceAtom.vaultId,
    likesAtom.vaultId,
    bobAtom.vaultId,
  )
  await wait(tripleAliceLikesBob.hash)

  // ============================================================
  // Create nested triple: (Carol endorses (Alice likes Bob))
  // The object is a triple, not an atom
  // ============================================================
  const tripleCarolEndorses = await alice.getCreateOrDepositOnTriple(
    carolAtom.vaultId,
    endorsesAtom.vaultId,
    tripleAliceLikesBob.vaultId, // <-- nested: object is a triple
  )
  await wait(tripleCarolEndorses.hash)

  // ============================================================
  // Create double-nested triple: (Dave follows (Carol endorses (Alice likes Bob)))
  // ============================================================
  const tripleDaveFollows = await alice.getCreateOrDepositOnTriple(
    daveAtom.vaultId,
    followsAtom.vaultId,
    tripleCarolEndorses.vaultId, // <-- double nested
  )
  await wait(tripleDaveFollows.hash)

  // Wait for triggers/aggregations to settle
  await new Promise(resolve => setTimeout(resolve, 3000))

  // ============================================================
  // Test 1: Base triple label is correct (atom-only, no nesting)
  // ============================================================
  test('base triple term_text label is correct', async () => {
    const result = await executeRaw<{
      triples: Array<{ term_id: string; term: { id: string } }>
    }>(`
      query GetTripleTermText($termId: String!) {
        triples(where: { term_id: { _eq: $termId } }) {
          term_id
          term {
            id
          }
        }
      }
    `, { termId: tripleAliceLikesBob.vaultId })

    expect(result.triples).toHaveLength(1)
    expect(result.triples[0].term_id).toBe(tripleAliceLikesBob.vaultId)

    // Query term_text directly to verify the label
    const termText = await executeRaw<{
      term_texts: Array<{ id: string; title: string; type: string }>
    }>(`
      query GetTermText($id: String!) {
        term_texts(where: { id: { _eq: $id } }) {
          id
          title
          type
        }
      }
    `, { id: tripleAliceLikesBob.vaultId })

    expect(termText.term_texts).toHaveLength(1)
    expect(termText.term_texts[0].type).toBe('triple')
    expect(termText.term_texts[0].title).toBe('Alice likes Bob')
  })

  // ============================================================
  // Test 2: Nested triple label includes parenthesized inner triple
  // ============================================================
  test('nested triple term_text label includes parenthesized inner triple', async () => {
    const termText = await executeRaw<{
      term_texts: Array<{ id: string; title: string; type: string }>
    }>(`
      query GetTermText($id: String!) {
        term_texts(where: { id: { _eq: $id } }) {
          id
          title
          type
        }
      }
    `, { id: tripleCarolEndorses.vaultId })

    expect(termText.term_texts).toHaveLength(1)
    expect(termText.term_texts[0].type).toBe('triple')
    expect(termText.term_texts[0].title).toBe('Carol endorses (Alice likes Bob)')
  })

  // ============================================================
  // Test 3: Double-nested triple label
  // ============================================================
  test('double-nested triple term_text label', async () => {
    const termText = await executeRaw<{
      term_texts: Array<{ id: string; title: string; type: string }>
    }>(`
      query GetTermText($id: String!) {
        term_texts(where: { id: { _eq: $id } }) {
          id
          title
          type
        }
      }
    `, { id: tripleDaveFollows.vaultId })

    expect(termText.term_texts).toHaveLength(1)
    expect(termText.term_texts[0].title).toBe('Dave follows (Carol endorses (Alice likes Bob))')
  })

  // ============================================================
  // Test 4: Old subject/predicate/object relationships still work for base triple
  // ============================================================
  test('old atom relationships work for base triple', async () => {
    const result = await executeRaw<{
      triple: {
        term_id: string
        subject: { label: string } | null
        predicate: { label: string } | null
        object: { label: string } | null
      } | null
    }>(`
      query GetBaseTripleAtomRels($termId: String!) {
        triple(term_id: $termId) {
          term_id
          subject {
            label
          }
          predicate {
            label
          }
          object {
            label
          }
        }
      }
    `, { termId: tripleAliceLikesBob.vaultId })

    expect(result.triple).not.toBeNull()
    expect(result.triple!.subject?.label).toBe('Alice')
    expect(result.triple!.predicate?.label).toBe('likes')
    expect(result.triple!.object?.label).toBe('Bob')
  })

  // ============================================================
  // Test 5: Old object relationship returns null for nested triple
  //         (object is a triple, not an atom)
  // ============================================================
  test('old object relationship returns null when object is a triple', async () => {
    const result = await executeRaw<{
      triple: {
        term_id: string
        subject: { label: string } | null
        predicate: { label: string } | null
        object: { label: string } | null
      } | null
    }>(`
      query GetNestedTripleOldRels($termId: String!) {
        triple(term_id: $termId) {
          term_id
          subject {
            label
          }
          predicate {
            label
          }
          object {
            label
          }
        }
      }
    `, { termId: tripleCarolEndorses.vaultId })

    expect(result.triple).not.toBeNull()
    // Subject and predicate are atoms — should resolve
    expect(result.triple!.subject?.label).toBe('Carol')
    expect(result.triple!.predicate?.label).toBe('endorses')
    // Object is a triple — old atom relationship should return null
    expect(result.triple!.object).toBeNull()
  })

  // ============================================================
  // Test 6: New subject_term/predicate_term/object_term relationships
  //         for a base triple — all resolve to atoms
  // ============================================================
  test('new term relationships resolve atoms for base triple', async () => {
    const result = await executeRaw<{
      triple: {
        subject_term: { id: string; type: string; atom: { label: string } | null; triple: any | null }
        predicate_term: { id: string; type: string; atom: { label: string } | null; triple: any | null }
        object_term: { id: string; type: string; atom: { label: string } | null; triple: any | null }
      } | null
    }>(`
      query GetBaseTripleTermRels($termId: String!) {
        triple(term_id: $termId) {
          subject_term {
            id
            type
            atom {
              label
            }
            triple {
              term_id
            }
          }
          predicate_term {
            id
            type
            atom {
              label
            }
            triple {
              term_id
            }
          }
          object_term {
            id
            type
            atom {
              label
            }
            triple {
              term_id
            }
          }
        }
      }
    `, { termId: tripleAliceLikesBob.vaultId })

    expect(result.triple).not.toBeNull()

    // All components are atoms
    expect(result.triple!.subject_term.type).toBe('Atom')
    expect(result.triple!.subject_term.atom?.label).toBe('Alice')
    expect(result.triple!.subject_term.triple).toBeNull()

    expect(result.triple!.predicate_term.type).toBe('Atom')
    expect(result.triple!.predicate_term.atom?.label).toBe('likes')

    expect(result.triple!.object_term.type).toBe('Atom')
    expect(result.triple!.object_term.atom?.label).toBe('Bob')
    expect(result.triple!.object_term.triple).toBeNull()
  })

  // ============================================================
  // Test 7: New term relationships for nested triple — object resolves to triple
  // ============================================================
  test('new term relationships resolve triple for nested triple object', async () => {
    const result = await executeRaw<{
      triple: {
        subject_term: { type: string; atom: { label: string } | null }
        predicate_term: { type: string; atom: { label: string } | null }
        object_term: {
          type: string
          atom: any | null
          triple: {
            term_id: string
            subject: { label: string } | null
            predicate: { label: string } | null
            object: { label: string } | null
          } | null
        }
      } | null
    }>(`
      query GetNestedTripleTermRels($termId: String!) {
        triple(term_id: $termId) {
          subject_term {
            type
            atom {
              label
            }
          }
          predicate_term {
            type
            atom {
              label
            }
          }
          object_term {
            type
            atom {
              label
            }
            triple {
              term_id
              subject {
                label
              }
              predicate {
                label
              }
              object {
                label
              }
            }
          }
        }
      }
    `, { termId: tripleCarolEndorses.vaultId })

    expect(result.triple).not.toBeNull()

    // Subject and predicate are atoms
    expect(result.triple!.subject_term.type).toBe('Atom')
    expect(result.triple!.subject_term.atom?.label).toBe('Carol')

    expect(result.triple!.predicate_term.type).toBe('Atom')
    expect(result.triple!.predicate_term.atom?.label).toBe('endorses')

    // Object is a triple
    expect(result.triple!.object_term.type).toBe('Triple')
    expect(result.triple!.object_term.atom).toBeNull()
    expect(result.triple!.object_term.triple).not.toBeNull()
    expect(result.triple!.object_term.triple!.term_id).toBe(tripleAliceLikesBob.vaultId)
    expect(result.triple!.object_term.triple!.subject?.label).toBe('Alice')
    expect(result.triple!.object_term.triple!.predicate?.label).toBe('likes')
    expect(result.triple!.object_term.triple!.object?.label).toBe('Bob')
  })

  // ============================================================
  // Test 8: Deep navigation — double nested triple via term relationships
  // ============================================================
  test('deep navigation through double-nested triple', async () => {
    const result = await executeRaw<{
      triple: {
        object_term: {
          type: string
          triple: {
            term_id: string
            object_term: {
              type: string
              triple: {
                term_id: string
                subject: { label: string } | null
                predicate: { label: string } | null
                object: { label: string } | null
              } | null
            }
          } | null
        }
      } | null
    }>(`
      query DeepNestedNavigation($termId: String!) {
        triple(term_id: $termId) {
          object_term {
            type
            triple {
              term_id
              object_term {
                type
                triple {
                  term_id
                  subject {
                    label
                  }
                  predicate {
                    label
                  }
                  object {
                    label
                  }
                }
              }
            }
          }
        }
      }
    `, { termId: tripleDaveFollows.vaultId })

    expect(result.triple).not.toBeNull()

    // First level: object of (Dave follows X) → (Carol endorses (Alice likes Bob))
    const level1 = result.triple!.object_term
    expect(level1.type).toBe('Triple')
    expect(level1.triple!.term_id).toBe(tripleCarolEndorses.vaultId)

    // Second level: object of (Carol endorses X) → (Alice likes Bob)
    const level2 = level1.triple!.object_term
    expect(level2.type).toBe('Triple')
    expect(level2.triple!.term_id).toBe(tripleAliceLikesBob.vaultId)
    expect(level2.triple!.subject?.label).toBe('Alice')
    expect(level2.triple!.predicate?.label).toBe('likes')
    expect(level2.triple!.object?.label).toBe('Bob')
  })

  // ============================================================
  // Test 9: Positions work on nested triples
  // ============================================================
  test('can deposit and query positions on nested triples', async () => {
    // Bob deposits on the nested triple (Carol endorses (Alice likes Bob))
    const deposit = await bob.getCreateOrDepositOnTriple(
      carolAtom.vaultId,
      endorsesAtom.vaultId,
      tripleAliceLikesBob.vaultId,
    )
    await wait(deposit.hash)

    // Wait for aggregations
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Query positions for the nested triple
    const result = await executeRaw<{
      positions: Array<{
        account_id: string
        term_id: string
        shares: string
      }>
    }>(`
      query NestedTriplePositions($termId: String!, $address: String!) {
        positions(where: {
          term_id: { _eq: $termId },
          account_id: { _eq: $address }
        }) {
          account_id
          term_id
          shares
        }
      }
    `, { termId: tripleCarolEndorses.vaultId, address: bob.account.address })

    expect(result.positions.length).toBeGreaterThan(0)
    expect(BigInt(result.positions[0].shares)).toBeGreaterThan(0n)
  })

  // ============================================================
  // Test 10: Term table correctly stores type for nested triple components
  // ============================================================
  test('term table has correct types for all components', async () => {
    const result = await executeRaw<{
      atom_term: { id: string; type: string } | null
      triple_term: { id: string; type: string } | null
    }>(`
      query TermTypes($atomId: String!, $tripleId: String!) {
        atom_term: term(id: $atomId) {
          id
          type
        }
        triple_term: term(id: $tripleId) {
          id
          type
        }
      }
    `, {
      atomId: aliceAtom.vaultId,
      tripleId: tripleAliceLikesBob.vaultId,
    })

    expect(result.atom_term).not.toBeNull()
    expect(result.atom_term!.type).toBe('Atom')

    expect(result.triple_term).not.toBeNull()
    expect(result.triple_term!.type).toBe('Triple')
  })

  // ============================================================
  // Test 11: Nested triple with triple as subject
  // ============================================================
  test('can create and query triple with nested subject', async () => {
    // Create: ((Alice likes Bob) endorses Carol)
    // Subject is a triple, predicate and object are atoms
    const tripleWithNestedSubject = await alice.getCreateOrDepositOnTriple(
      tripleAliceLikesBob.vaultId, // <-- subject is a triple
      endorsesAtom.vaultId,
      carolAtom.vaultId,
    )
    await wait(tripleWithNestedSubject.hash)
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Verify label
    const termText = await executeRaw<{
      term_texts: Array<{ title: string }>
    }>(`
      query GetTermText($id: String!) {
        term_texts(where: { id: { _eq: $id } }) {
          title
        }
      }
    `, { id: tripleWithNestedSubject.vaultId })

    expect(termText.term_texts).toHaveLength(1)
    expect(termText.term_texts[0].title).toBe('(Alice likes Bob) endorses Carol')

    // Verify subject_term resolves to triple
    const result = await executeRaw<{
      triple: {
        subject_term: { type: string; triple: { term_id: string } | null }
        object_term: { type: string; atom: { label: string } | null }
      } | null
    }>(`
      query NestedSubjectRels($termId: String!) {
        triple(term_id: $termId) {
          subject_term {
            type
            triple {
              term_id
            }
          }
          object_term {
            type
            atom {
              label
            }
          }
        }
      }
    `, { termId: tripleWithNestedSubject.vaultId })

    expect(result.triple!.subject_term.type).toBe('Triple')
    expect(result.triple!.subject_term.triple!.term_id).toBe(tripleAliceLikesBob.vaultId)
    expect(result.triple!.object_term.type).toBe('Atom')
    expect(result.triple!.object_term.atom?.label).toBe('Carol')
  })

  // ============================================================
  // Test 12: Nested triple with triple as predicate
  // ============================================================
  test('can create and query triple with nested predicate', async () => {
    // Create: (Alice (Alice likes Bob) Carol)
    // Predicate is a triple — unusual but valid
    const tripleWithNestedPredicate = await alice.getCreateOrDepositOnTriple(
      aliceAtom.vaultId,
      tripleAliceLikesBob.vaultId, // <-- predicate is a triple
      carolAtom.vaultId,
    )
    await wait(tripleWithNestedPredicate.hash)
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Verify label
    const termText = await executeRaw<{
      term_texts: Array<{ title: string }>
    }>(`
      query GetTermText($id: String!) {
        term_texts(where: { id: { _eq: $id } }) {
          title
        }
      }
    `, { id: tripleWithNestedPredicate.vaultId })

    expect(termText.term_texts).toHaveLength(1)
    expect(termText.term_texts[0].title).toBe('Alice (Alice likes Bob) Carol')

    // Verify predicate_term resolves to triple
    const result = await executeRaw<{
      triple: {
        predicate_term: { type: string; triple: { term_id: string } | null }
      } | null
    }>(`
      query NestedPredicateRels($termId: String!) {
        triple(term_id: $termId) {
          predicate_term {
            type
            triple {
              term_id
            }
          }
        }
      }
    `, { termId: tripleWithNestedPredicate.vaultId })

    expect(result.triple!.predicate_term.type).toBe('Triple')
    expect(result.triple!.predicate_term.triple!.term_id).toBe(tripleAliceLikesBob.vaultId)
  })

  // ============================================================
  // Test 13: All three components are triples
  // ============================================================
  test('can create triple where all components are triples', async () => {
    // Create two more base triples for subject and predicate
    const tripleCarolLikesDave = await alice.getCreateOrDepositOnTriple(
      carolAtom.vaultId,
      likesAtom.vaultId,
      daveAtom.vaultId,
    )
    await wait(tripleCarolLikesDave.hash)

    const tripleBobFollowsAlice = await alice.getCreateOrDepositOnTriple(
      bobAtom.vaultId,
      followsAtom.vaultId,
      aliceAtom.vaultId,
    )
    await wait(tripleBobFollowsAlice.hash)

    // Create: ((Carol likes Dave) (Bob follows Alice) (Alice likes Bob))
    // All three components are triples
    const allTripleComponents = await alice.getCreateOrDepositOnTriple(
      tripleCarolLikesDave.vaultId,
      tripleBobFollowsAlice.vaultId,
      tripleAliceLikesBob.vaultId,
    )
    await wait(allTripleComponents.hash)
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Verify label
    const termText = await executeRaw<{
      term_texts: Array<{ title: string }>
    }>(`
      query GetTermText($id: String!) {
        term_texts(where: { id: { _eq: $id } }) {
          title
        }
      }
    `, { id: allTripleComponents.vaultId })

    expect(termText.term_texts).toHaveLength(1)
    expect(termText.term_texts[0].title).toBe(
      '(Carol likes Dave) (Bob follows Alice) (Alice likes Bob)'
    )

    // All old atom relationships should return null
    const oldRels = await executeRaw<{
      triple: {
        subject: any | null
        predicate: any | null
        object: any | null
      } | null
    }>(`
      query AllTripleComponentsOldRels($termId: String!) {
        triple(term_id: $termId) {
          subject { label }
          predicate { label }
          object { label }
        }
      }
    `, { termId: allTripleComponents.vaultId })

    expect(oldRels.triple!.subject).toBeNull()
    expect(oldRels.triple!.predicate).toBeNull()
    expect(oldRels.triple!.object).toBeNull()

    // All new term relationships should resolve to Triple type
    const newRels = await executeRaw<{
      triple: {
        subject_term: { type: string }
        predicate_term: { type: string }
        object_term: { type: string }
      } | null
    }>(`
      query AllTripleComponentsNewRels($termId: String!) {
        triple(term_id: $termId) {
          subject_term { type }
          predicate_term { type }
          object_term { type }
        }
      }
    `, { termId: allTripleComponents.vaultId })

    expect(newRels.triple!.subject_term.type).toBe('Triple')
    expect(newRels.triple!.predicate_term.type).toBe('Triple')
    expect(newRels.triple!.object_term.type).toBe('Triple')
  })

  // ============================================================
  // Test 14: Existing triples (atom-only) are unaffected
  // ============================================================
  test('existing atom-only triples work correctly after migration', async () => {
    // Create a plain triple with no nesting
    const eveAtom = await alice.getOrCreateAtom(await pinJson({
      '@context': 'https://schema.org',
      '@type': 'Thing',
      name: 'Eve',
      description: 'Regular atom for backward compatibility test',
    }))
    await wait(eveAtom.hash)

    const regularTriple = await alice.getCreateOrDepositOnTriple(
      eveAtom.vaultId,
      likesAtom.vaultId,
      carolAtom.vaultId,
    )
    await wait(regularTriple.hash)
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Old relationships work
    const oldResult = await executeRaw<{
      triple: {
        subject: { label: string } | null
        predicate: { label: string } | null
        object: { label: string } | null
      } | null
    }>(`
      query RegularTripleOldRels($termId: String!) {
        triple(term_id: $termId) {
          subject { label }
          predicate { label }
          object { label }
        }
      }
    `, { termId: regularTriple.vaultId })

    expect(oldResult.triple!.subject?.label).toBe('Eve')
    expect(oldResult.triple!.predicate?.label).toBe('likes')
    expect(oldResult.triple!.object?.label).toBe('Carol')

    // New term relationships also work for atom-only triples
    const newResult = await executeRaw<{
      triple: {
        subject_term: { type: string; atom: { label: string } | null }
        predicate_term: { type: string; atom: { label: string } | null }
        object_term: { type: string; atom: { label: string } | null }
      } | null
    }>(`
      query RegularTripleNewRels($termId: String!) {
        triple(term_id: $termId) {
          subject_term { type atom { label } }
          predicate_term { type atom { label } }
          object_term { type atom { label } }
        }
      }
    `, { termId: regularTriple.vaultId })

    expect(newResult.triple!.subject_term.type).toBe('Atom')
    expect(newResult.triple!.subject_term.atom?.label).toBe('Eve')
    expect(newResult.triple!.predicate_term.type).toBe('Atom')
    expect(newResult.triple!.object_term.type).toBe('Atom')
    expect(newResult.triple!.object_term.atom?.label).toBe('Carol')

    // Label is correct
    const termText = await executeRaw<{
      term_texts: Array<{ title: string }>
    }>(`
      query GetTermText($id: String!) {
        term_texts(where: { id: { _eq: $id } }) {
          title
        }
      }
    `, { id: regularTriple.vaultId })

    expect(termText.term_texts[0].title).toBe('Eve likes Carol')
  })
})
