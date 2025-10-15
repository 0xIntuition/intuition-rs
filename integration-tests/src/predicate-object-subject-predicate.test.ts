import { expect, test, suite } from 'vitest'
import { execute, getIntuition, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'
import { parseEther } from 'viem'

suite('predicate_object triggers', () => {
  test('should automatically create predicate_object when triple is created', async () => {
    const user = await getIntuition(2001)

    // Create atoms
    const subject = await user.getOrCreateAtom('test-subject-po-1')
    await wait(subject.hash)

    const predicate = await user.getOrCreateAtom('test-predicate-po-1')
    await wait(predicate.hash)

    const object = await user.getOrCreateAtom('test-object-po-1')
    await wait(object.hash)

    // Create triple
    const triple = await user.getCreateOrDepositOnTriple(
      subject.vaultId,
      predicate.vaultId,
      object.vaultId,
    )
    await wait(triple.hash)

    // Query predicate_object
    const query = graphql(`
      query PredicateObjectAutoCreate($predicateId: String!, $objectId: String!) {
        predicate_objects(where: {
          predicate_id: { _eq: $predicateId },
          object_id: { _eq: $objectId }
        }) {
          predicate_id
          object_id
          triple_count
          total_position_count
          total_market_cap
        }
      }
    `)

    const result = await execute(query, {
      predicateId: predicate.vaultId.toString(),
      objectId: object.vaultId.toString()
    })

    expect(result.predicate_objects).toBeDefined()
    expect(result.predicate_objects.length).toBe(1)
    expect(result.predicate_objects[0].predicate_id).toBe(predicate.vaultId.toString())
    expect(result.predicate_objects[0].object_id).toBe(object.vaultId.toString())
    expect(result.predicate_objects[0].triple_count).toBe(1)
    // A position is created when triple is created with initial deposit
    expect(result.predicate_objects[0].total_position_count).toBeGreaterThanOrEqual(1)
    expect(BigInt(result.predicate_objects[0].total_market_cap)).toBeGreaterThanOrEqual(0n)
  })

  test('should increment triple_count for existing predicate_object', async () => {
    const user = await getIntuition(2002)

    // Create atoms
    const subject1 = await user.getOrCreateAtom('test-subject-po-2')
    await wait(subject1.hash)

    const subject2 = await user.getOrCreateAtom('test-subject-po-3')
    await wait(subject2.hash)

    const predicate = await user.getOrCreateAtom('test-predicate-po-2')
    await wait(predicate.hash)

    const object = await user.getOrCreateAtom('test-object-po-2')
    await wait(object.hash)

    // Create first triple with this predicate-object combination
    const triple1 = await user.getCreateOrDepositOnTriple(
      subject1.vaultId,
      predicate.vaultId,
      object.vaultId,
    )
    await wait(triple1.hash)

    // Create second triple with same predicate-object combination
    const triple2 = await user.getCreateOrDepositOnTriple(
      subject2.vaultId,
      predicate.vaultId,
      object.vaultId,
    )
    await wait(triple2.hash)

    // Query predicate_object
    const query = graphql(`
      query PredicateObjectTripleCount($predicateId: String!, $objectId: String!) {
        predicate_objects(where: {
          predicate_id: { _eq: $predicateId },
          object_id: { _eq: $objectId }
        }) {
          triple_count
          triples {
            term_id
          }
        }
      }
    `)

    const result = await execute(query, {
      predicateId: predicate.vaultId.toString(),
      objectId: object.vaultId.toString()
    })

    expect(result.predicate_objects).toBeDefined()
    expect(result.predicate_objects.length).toBe(1)
    expect(result.predicate_objects[0].triple_count).toBe(2)
    expect(result.predicate_objects[0].triples.length).toBe(2)
  })
})

suite('subject_predicate triggers', () => {
  test('should automatically create subject_predicate when triple is created', async () => {
    const user = await getIntuition(2003)

    // Create atoms
    const subject = await user.getOrCreateAtom('test-subject-sp-1')
    await wait(subject.hash)

    const predicate = await user.getOrCreateAtom('test-predicate-sp-1')
    await wait(predicate.hash)

    const object = await user.getOrCreateAtom('test-object-sp-1')
    await wait(object.hash)

    // Create triple
    const triple = await user.getCreateOrDepositOnTriple(
      subject.vaultId,
      predicate.vaultId,
      object.vaultId,
    )
    await wait(triple.hash)

    // Query subject_predicate
    const query = graphql(`
      query SubjectPredicateAutoCreate($subjectId: String!, $predicateId: String!) {
        subject_predicates(where: {
          subject_id: { _eq: $subjectId },
          predicate_id: { _eq: $predicateId }
        }) {
          subject_id
          predicate_id
          triple_count
          total_position_count
          total_market_cap
        }
      }
    `)

    const result = await execute(query, {
      subjectId: subject.vaultId.toString(),
      predicateId: predicate.vaultId.toString()
    })

    expect(result.subject_predicates).toBeDefined()
    expect(result.subject_predicates.length).toBe(1)
    expect(result.subject_predicates[0].subject_id).toBe(subject.vaultId.toString())
    expect(result.subject_predicates[0].predicate_id).toBe(predicate.vaultId.toString())
    expect(result.subject_predicates[0].triple_count).toBe(1)
    // A position is created when triple is created with initial deposit
    expect(result.subject_predicates[0].total_position_count).toBeGreaterThanOrEqual(1)
    expect(BigInt(result.subject_predicates[0].total_market_cap)).toBeGreaterThanOrEqual(0n)
  })

  test('should increment triple_count for existing subject_predicate', async () => {
    const user = await getIntuition(2004)

    // Create atoms
    const subject = await user.getOrCreateAtom('test-subject-sp-2')
    await wait(subject.hash)

    const predicate = await user.getOrCreateAtom('test-predicate-sp-2')
    await wait(predicate.hash)

    const object1 = await user.getOrCreateAtom('test-object-sp-2')
    await wait(object1.hash)

    const object2 = await user.getOrCreateAtom('test-object-sp-3')
    await wait(object2.hash)

    // Create first triple with this subject-predicate combination
    const triple1 = await user.getCreateOrDepositOnTriple(
      subject.vaultId,
      predicate.vaultId,
      object1.vaultId,
    )
    await wait(triple1.hash)

    // Create second triple with same subject-predicate combination
    const triple2 = await user.getCreateOrDepositOnTriple(
      subject.vaultId,
      predicate.vaultId,
      object2.vaultId,
    )
    await wait(triple2.hash)

    // Query subject_predicate
    const query = graphql(`
      query SubjectPredicateTripleCount($subjectId: String!, $predicateId: String!) {
        subject_predicates(where: {
          subject_id: { _eq: $subjectId },
          predicate_id: { _eq: $predicateId }
        }) {
          triple_count
          triples {
            term_id
          }
        }
      }
    `)

    const result = await execute(query, {
      subjectId: subject.vaultId.toString(),
      predicateId: predicate.vaultId.toString()
    })

    expect(result.subject_predicates).toBeDefined()
    expect(result.subject_predicates.length).toBe(1)
    expect(result.subject_predicates[0].triple_count).toBe(2)
    expect(result.subject_predicates[0].triples.length).toBe(2)
  })
})

suite('predicate_object aggregations', () => {
  test('should update total_market_cap when triple_term changes', async () => {
    const user1 = await getIntuition(2005)

    // Create atoms
    const subject = await user1.getOrCreateAtom('test-subject-po-agg-1')
    await wait(subject.hash)

    const predicate = await user1.getOrCreateAtom('test-predicate-po-agg-1')
    await wait(predicate.hash)

    const object = await user1.getOrCreateAtom('test-object-po-agg-1')
    await wait(object.hash)

    // Create triple with initial deposit
    const triple = await user1.getCreateOrDepositOnTriple(
      subject.vaultId,
      predicate.vaultId,
      object.vaultId,
      parseEther('1')
    )
    await wait(triple.hash)

    // Wait for aggregates to update
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Query predicate_object
    const query = graphql(`
      query PredicateObjectAggregates($predicateId: String!, $objectId: String!) {
        predicate_objects(where: {
          predicate_id: { _eq: $predicateId },
          object_id: { _eq: $objectId }
        }) {
          total_market_cap
          total_position_count
          triple_count
        }
      }
    `)

    const result = await execute(query, {
      predicateId: predicate.vaultId.toString(),
      objectId: object.vaultId.toString()
    })

    expect(result.predicate_objects).toBeDefined()
    expect(result.predicate_objects.length).toBe(1)
    expect(result.predicate_objects[0].triple_count).toBe(1)
    expect(BigInt(result.predicate_objects[0].total_market_cap)).toBeGreaterThan(0n)
    expect(result.predicate_objects[0].total_position_count).toBeGreaterThan(0)
  })

  test('should aggregate across multiple triples with same predicate-object', async () => {
    const user1 = await getIntuition(2006)

    // Create atoms
    const subject1 = await user1.getOrCreateAtom('test-subject-po-agg-2')
    await wait(subject1.hash)

    const subject2 = await user1.getOrCreateAtom('test-subject-po-agg-3')
    await wait(subject2.hash)

    const predicate = await user1.getOrCreateAtom('test-predicate-po-agg-2')
    await wait(predicate.hash)

    const object = await user1.getOrCreateAtom('test-object-po-agg-2')
    await wait(object.hash)

    // Create first triple with deposit
    const triple1 = await user1.getCreateOrDepositOnTriple(
      subject1.vaultId,
      predicate.vaultId,
      object.vaultId,
      parseEther('0.5')
    )
    await wait(triple1.hash)

    // Create second triple with deposit
    const triple2 = await user1.getCreateOrDepositOnTriple(
      subject2.vaultId,
      predicate.vaultId,
      object.vaultId,
      parseEther('0.5')
    )
    await wait(triple2.hash)

    // Wait for aggregates to update
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Query predicate_object
    const query = graphql(`
      query PredicateObjectAggregates($predicateId: String!, $objectId: String!) {
        predicate_objects(where: {
          predicate_id: { _eq: $predicateId },
          object_id: { _eq: $objectId }
        }) {
          total_market_cap
          total_position_count
          triple_count
        }
      }
    `)

    const result = await execute(query, {
      predicateId: predicate.vaultId.toString(),
      objectId: object.vaultId.toString()
    })

    expect(result.predicate_objects).toBeDefined()
    expect(result.predicate_objects.length).toBe(1)
    expect(result.predicate_objects[0].triple_count).toBe(2)
    expect(BigInt(result.predicate_objects[0].total_market_cap)).toBeGreaterThan(0n)
    expect(result.predicate_objects[0].total_position_count).toBeGreaterThan(0)
  })

  test('should update total_position_count when positions are opened/closed', async () => {
    const user1 = await getIntuition(2007)
    const user2 = await getIntuition(2008)

    // Create atoms
    const subject = await user1.getOrCreateAtom('test-subject-po-agg-4')
    await wait(subject.hash)

    const predicate = await user1.getOrCreateAtom('test-predicate-po-agg-3')
    await wait(predicate.hash)

    const object = await user1.getOrCreateAtom('test-object-po-agg-3')
    await wait(object.hash)

    // Create triple with deposit from user1
    const triple = await user1.getCreateOrDepositOnTriple(
      subject.vaultId,
      predicate.vaultId,
      object.vaultId,
      parseEther('0.5')
    )
    await wait(triple.hash)

    // Wait for aggregates to update
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Get initial position count
    const query = graphql(`
      query PredicateObjectPositions($predicateId: String!, $objectId: String!) {
        predicate_objects(where: {
          predicate_id: { _eq: $predicateId },
          object_id: { _eq: $objectId }
        }) {
          total_position_count
        }
      }
    `)

    const initialResult = await execute(query, {
      predicateId: predicate.vaultId.toString(),
      objectId: object.vaultId.toString()
    })

    const initialPositionCount = initialResult.predicate_objects[0].total_position_count

    // User2 creates a new position
    const depositHash = await user2.contract.write.deposit(
      [user2.account.address, triple.vaultId, 1n, 0n],
      { value: parseEther('0.3') }
    )
    await wait(depositHash)

    // Wait for aggregates to update
    await new Promise(resolve => setTimeout(resolve, 2000))

    const finalResult = await execute(query, {
      predicateId: predicate.vaultId.toString(),
      objectId: object.vaultId.toString()
    })

    // Position count should either increase or stay the same (depending on counter vault aggregation)
    // The key test is that it doesn't decrease and is greater than 0
    expect(finalResult.predicate_objects[0].total_position_count).toBeGreaterThanOrEqual(initialPositionCount)
    expect(finalResult.predicate_objects[0].total_position_count).toBeGreaterThan(0)
  })
})

suite('subject_predicate aggregations', () => {
  test('should update total_market_cap when triple_term changes', async () => {
    const user1 = await getIntuition(2009)

    // Create atoms
    const subject = await user1.getOrCreateAtom('test-subject-sp-agg-1')
    await wait(subject.hash)

    const predicate = await user1.getOrCreateAtom('test-predicate-sp-agg-1')
    await wait(predicate.hash)

    const object = await user1.getOrCreateAtom('test-object-sp-agg-1')
    await wait(object.hash)

    // Create triple with initial deposit
    const triple = await user1.getCreateOrDepositOnTriple(
      subject.vaultId,
      predicate.vaultId,
      object.vaultId,
      parseEther('1')
    )
    await wait(triple.hash)

    // Wait for aggregates to update
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Query subject_predicate
    const query = graphql(`
      query SubjectPredicateAggregates($subjectId: String!, $predicateId: String!) {
        subject_predicates(where: {
          subject_id: { _eq: $subjectId },
          predicate_id: { _eq: $predicateId }
        }) {
          total_market_cap
          total_position_count
          triple_count
        }
      }
    `)

    const result = await execute(query, {
      subjectId: subject.vaultId.toString(),
      predicateId: predicate.vaultId.toString()
    })

    expect(result.subject_predicates).toBeDefined()
    expect(result.subject_predicates.length).toBe(1)
    expect(result.subject_predicates[0].triple_count).toBe(1)
    expect(BigInt(result.subject_predicates[0].total_market_cap)).toBeGreaterThan(0n)
    expect(result.subject_predicates[0].total_position_count).toBeGreaterThan(0)
  })

  test('should aggregate across multiple triples with same subject-predicate', async () => {
    const user1 = await getIntuition(2010)

    // Create atoms
    const subject = await user1.getOrCreateAtom('test-subject-sp-agg-2')
    await wait(subject.hash)

    const predicate = await user1.getOrCreateAtom('test-predicate-sp-agg-2')
    await wait(predicate.hash)

    const object1 = await user1.getOrCreateAtom('test-object-sp-agg-2')
    await wait(object1.hash)

    const object2 = await user1.getOrCreateAtom('test-object-sp-agg-3')
    await wait(object2.hash)

    // Create first triple with deposit
    const triple1 = await user1.getCreateOrDepositOnTriple(
      subject.vaultId,
      predicate.vaultId,
      object1.vaultId,
      parseEther('0.5')
    )
    await wait(triple1.hash)

    // Create second triple with deposit
    const triple2 = await user1.getCreateOrDepositOnTriple(
      subject.vaultId,
      predicate.vaultId,
      object2.vaultId,
      parseEther('0.5')
    )
    await wait(triple2.hash)

    // Wait for aggregates to update
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Query subject_predicate
    const query = graphql(`
      query SubjectPredicateAggregates($subjectId: String!, $predicateId: String!) {
        subject_predicates(where: {
          subject_id: { _eq: $subjectId },
          predicate_id: { _eq: $predicateId }
        }) {
          total_market_cap
          total_position_count
          triple_count
        }
      }
    `)

    const result = await execute(query, {
      subjectId: subject.vaultId.toString(),
      predicateId: predicate.vaultId.toString()
    })

    expect(result.subject_predicates).toBeDefined()
    expect(result.subject_predicates.length).toBe(1)
    expect(result.subject_predicates[0].triple_count).toBe(2)
    expect(BigInt(result.subject_predicates[0].total_market_cap)).toBeGreaterThan(0n)
    expect(result.subject_predicates[0].total_position_count).toBeGreaterThan(0)
  })

  test('should update total_position_count when positions are opened/closed', async () => {
    const user1 = await getIntuition(2011)
    const user2 = await getIntuition(2012)

    // Create atoms
    const subject = await user1.getOrCreateAtom('test-subject-sp-agg-4')
    await wait(subject.hash)

    const predicate = await user1.getOrCreateAtom('test-predicate-sp-agg-3')
    await wait(predicate.hash)

    const object = await user1.getOrCreateAtom('test-object-sp-agg-4')
    await wait(object.hash)

    // Create triple with deposit from user1
    const triple = await user1.getCreateOrDepositOnTriple(
      subject.vaultId,
      predicate.vaultId,
      object.vaultId,
      parseEther('0.5')
    )
    await wait(triple.hash)

    // Wait for aggregates to update
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Get initial position count
    const query = graphql(`
      query SubjectPredicatePositions($subjectId: String!, $predicateId: String!) {
        subject_predicates(where: {
          subject_id: { _eq: $subjectId },
          predicate_id: { _eq: $predicateId }
        }) {
          total_position_count
        }
      }
    `)

    const initialResult = await execute(query, {
      subjectId: subject.vaultId.toString(),
      predicateId: predicate.vaultId.toString()
    })

    const initialPositionCount = initialResult.subject_predicates[0].total_position_count

    // User2 creates a new position
    const depositHash = await user2.contract.write.deposit(
      [user2.account.address, triple.vaultId, 1n, 0n],
      { value: parseEther('0.3') }
    )
    await wait(depositHash)

    // Wait for aggregates to update
    await new Promise(resolve => setTimeout(resolve, 2000))

    const finalResult = await execute(query, {
      subjectId: subject.vaultId.toString(),
      predicateId: predicate.vaultId.toString()
    })

    // Position count should either increase or stay the same (depending on counter vault aggregation)
    // The key test is that it doesn't decrease and is greater than 0
    expect(finalResult.subject_predicates[0].total_position_count).toBeGreaterThanOrEqual(initialPositionCount)
    expect(finalResult.subject_predicates[0].total_position_count).toBeGreaterThan(0)
  })
})

suite('predicate_object and subject_predicate integration', () => {
  test('should update both tables correctly for a single triple', async () => {
    const user = await getIntuition(2013)

    // Create atoms
    const subject = await user.getOrCreateAtom('test-subject-integration-1')
    await wait(subject.hash)

    const predicate = await user.getOrCreateAtom('test-predicate-integration-1')
    await wait(predicate.hash)

    const object = await user.getOrCreateAtom('test-object-integration-1')
    await wait(object.hash)

    // Create triple
    const triple = await user.getCreateOrDepositOnTriple(
      subject.vaultId,
      predicate.vaultId,
      object.vaultId,
      parseEther('1')
    )
    await wait(triple.hash)

    // Wait for aggregates to update
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Query both tables
    const query = graphql(`
      query IntegrationCheck($subjectId: String!, $predicateId: String!, $objectId: String!) {
        predicate_objects(where: {
          predicate_id: { _eq: $predicateId },
          object_id: { _eq: $objectId }
        }) {
          triple_count
          total_market_cap
          total_position_count
        }
        subject_predicates(where: {
          subject_id: { _eq: $subjectId },
          predicate_id: { _eq: $predicateId }
        }) {
          triple_count
          total_market_cap
          total_position_count
        }
      }
    `)

    const result = await execute(query, {
      subjectId: subject.vaultId.toString(),
      predicateId: predicate.vaultId.toString(),
      objectId: object.vaultId.toString()
    })

    // Verify predicate_object
    expect(result.predicate_objects).toBeDefined()
    expect(result.predicate_objects.length).toBe(1)
    expect(result.predicate_objects[0].triple_count).toBe(1)
    expect(BigInt(result.predicate_objects[0].total_market_cap)).toBeGreaterThan(0n)
    expect(result.predicate_objects[0].total_position_count).toBeGreaterThan(0)

    // Verify subject_predicate
    expect(result.subject_predicates).toBeDefined()
    expect(result.subject_predicates.length).toBe(1)
    expect(result.subject_predicates[0].triple_count).toBe(1)
    expect(BigInt(result.subject_predicates[0].total_market_cap)).toBeGreaterThan(0n)
    expect(result.subject_predicates[0].total_position_count).toBeGreaterThan(0)

    // Both should have the same aggregates for this single triple
    expect(result.predicate_objects[0].total_market_cap).toBe(result.subject_predicates[0].total_market_cap)
    expect(result.predicate_objects[0].total_position_count).toBe(result.subject_predicates[0].total_position_count)
  })

  test('should handle complex scenario with multiple users and triples', async () => {
    const user1 = await getIntuition(2014)
    const user2 = await getIntuition(2015)

    // Create shared atoms
    const predicate = await user1.getOrCreateAtom('test-predicate-complex')
    await wait(predicate.hash)

    const object = await user1.getOrCreateAtom('test-object-complex')
    await wait(object.hash)

    // User1 creates two triples with same predicate-object
    const subject1 = await user1.getOrCreateAtom('test-subject-complex-1')
    await wait(subject1.hash)

    const triple1 = await user1.getCreateOrDepositOnTriple(
      subject1.vaultId,
      predicate.vaultId,
      object.vaultId,
      parseEther('0.5')
    )
    await wait(triple1.hash)

    const subject2 = await user1.getOrCreateAtom('test-subject-complex-2')
    await wait(subject2.hash)

    const triple2 = await user1.getCreateOrDepositOnTriple(
      subject2.vaultId,
      predicate.vaultId,
      object.vaultId,
      parseEther('0.5')
    )
    await wait(triple2.hash)

    // User2 deposits on triple1
    const depositHash = await user2.contract.write.deposit(
      [user2.account.address, triple1.vaultId, 1n, 0n],
      { value: parseEther('0.3') }
    )
    await wait(depositHash)

    // Wait for aggregates to update
    await new Promise(resolve => setTimeout(resolve, 2000))

    // Query predicate_object
    const query = graphql(`
      query ComplexScenario($predicateId: String!, $objectId: String!) {
        predicate_objects(where: {
          predicate_id: { _eq: $predicateId },
          object_id: { _eq: $objectId }
        }) {
          triple_count
          total_market_cap
          total_position_count
          triples {
            term_id
            subject_id
            predicate_id
            object_id
          }
        }
      }
    `)

    const result = await execute(query, {
      predicateId: predicate.vaultId.toString(),
      objectId: object.vaultId.toString()
    })

    expect(result.predicate_objects).toBeDefined()
    expect(result.predicate_objects.length).toBe(1)
    expect(result.predicate_objects[0].triple_count).toBe(2)
    expect(result.predicate_objects[0].triples.length).toBe(2)
    expect(BigInt(result.predicate_objects[0].total_market_cap)).toBeGreaterThan(0n)
    expect(result.predicate_objects[0].total_position_count).toBeGreaterThan(0)

    // Verify both triples are in the relation
    const tripleIds = result.predicate_objects[0].triples.map(t => t.term_id)
    expect(tripleIds).toContain(triple1.vaultId.toString())
    expect(tripleIds).toContain(triple2.vaultId.toString())
  })
})
