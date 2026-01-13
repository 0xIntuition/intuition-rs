# Product Requirements Document: Semantic Search for Triples

## 1. Overview

### 1.1 Purpose
Extend the existing semantic search functionality to include triples (relationships) in addition to atoms (entities), enabling users to search across the full knowledge graph using natural language queries.

### 1.2 Background
Currently, the semantic search only indexes and searches atoms using pgai with OpenAI embeddings stored in the `term_text` table. Triples represent relationships between atoms (subject-predicate-object) and contain valuable semantic meaning that should be searchable.

### 1.3 Goals
- Enable semantic search across triples using the existing `search_term` function
- Reuse existing infrastructure (`term_text` table, pgai embeddings, GraphQL API)
- Maintain backward compatibility with current atom search
- Backfill existing triples into the search index

## 2. Technical Requirements

### 2.1 Data Model

#### 2.1.1 term_text Table Extension
- **Current State**: Stores atom labels with auto-generated embeddings
- **Required Change**: Include triple entries with concatenated text
- **Format**: `"{subject.label} {predicate.label} {object.label}"`
- **ID Strategy**: Use same `term_id` logic for both atoms and triples (polymorphic)

#### 2.1.2 Triple Representation
- Triples already have denormalized text fields for subject/predicate/object labels
- No additional joins required during trigger execution
- Text format example: "Alice knows Bob" (where Alice=subject, knows=predicate, Bob=object)

### 2.2 Database Triggers

#### 2.2.1 INSERT Trigger
Create trigger on triple table to automatically insert into `term_text` when new triples are created:
```sql
-- Pseudo-code
ON INSERT INTO triples
  INSERT INTO term_text (term_id, text, type)
  VALUES (NEW.id, CONCAT(NEW.subject_label, ' ', NEW.predicate_label, ' ', NEW.object_label), 'triple')
```

#### 2.2.2 UPDATE Trigger
Create trigger to update `term_text` when triple components change:
```sql
-- Pseudo-code  
ON UPDATE OF subject_label, predicate_label, object_label ON triples
  UPDATE term_text
  SET text = CONCAT(NEW.subject_label, ' ', NEW.predicate_label, ' ', NEW.object_label)
  WHERE term_id = NEW.id AND type = 'triple'
```

#### 2.2.3 Cascade Considerations
- If atom labels change, need to update all triples referencing that atom
- Requires additional triggers on atom table or denormalization strategy
- **Decision needed**: How to handle atom label updates affecting triple text

### 2.3 Migration

#### 2.3.1 Backfill Script
Create migration to populate `term_text` with existing triples:
```sql
-- Pseudo-code
INSERT INTO term_text (term_id, text, type)
SELECT 
  id,
  CONCAT(subject_label, ' ', predicate_label, ' ', object_label),
  'triple'
FROM triples
WHERE id NOT IN (SELECT term_id FROM term_text WHERE type = 'triple')
```

#### 2.3.2 Migration Path
1. Add triggers (forward-looking)
2. Run backfill for historical data
3. Verify embedding generation via pgai
4. No application code changes required (reuses `search_term`)

### 2.4 API Integration

#### 2.4.1 GraphQL Query (No Changes Required)
Existing `search_term` function through Hasura will automatically return triple results:
```graphql
query SearchTerms($query: String!) {
  search_term(args: {query: $query}) {
    term_id
    text
    similarity_score
    type  # 'atom' or 'triple'
  }
}
```

#### 2.4.2 Result Format
- Same schema as atom search results
- Frontend will differentiate based on `type` field or by looking up the ID
- No breaking changes to existing clients

### 2.5 Performance Considerations

#### 2.5.1 Non-Critical Performance
- Search latency not critical for initial release
- Target: <1s response time acceptable
- Primarily for exploration and discovery use cases

#### 2.5.2 Embedding Generation
- Embeddings generated automatically by existing pgai integration
- Triggered when new rows added to `term_text`
- No additional infrastructure required

#### 2.5.3 Index Size
- Estimate triple count vs atom count
- Monitor `term_text` table growth
- pgai vector index performance with larger dataset

## 3. Functional Requirements

### 3.1 Search Behavior

#### 3.1.1 Semantic Understanding
- Search based on the relationship meaning represented by the triple
- Example: Query "friendship" should match triples like "Alice knows Bob"
- Leverages OpenAI embeddings for semantic similarity

#### 3.1.2 Mixed Results
- Single query returns both atoms and triples
- Results ranked by similarity score
- No separate "atom search" vs "triple search" - unified experience

### 3.2 Data Quality

#### 3.2.1 Text Format Consistency
- All triples formatted as: `"{subject.label} {predicate.label} {object.label}"`
- Preserve spacing and casing from source labels
- Handle null/empty labels gracefully

#### 3.2.2 Triple Updates
- Changes to triple components reflected in search index
- Embedding automatically regenerated on text update
- Maintain referential integrity with source triple data

## 4. Implementation Plan

### 4.1 Phase 1: Database Changes
1. **Add type discriminator** to `term_text` table if not present
   - `ALTER TABLE term_text ADD COLUMN IF NOT EXISTS type TEXT`
2. **Create INSERT trigger** on triples table
3. **Create UPDATE trigger** on triples table
4. **Test triggers** with manual INSERT/UPDATE operations

### 4.2 Phase 2: Backfill Migration
1. **Write migration script** to populate historical triples
2. **Run migration** in non-prod environment
3. **Verify embedding generation** - check pgai has processed all new rows
4. **Test search results** - query for known triples
5. **Deploy to production**

### 4.3 Phase 3: Validation & Monitoring
1. **Verify search results** include both atoms and triples
2. **Monitor embedding generation** latency and success rate
3. **Check vector index performance** with increased data volume
4. **Gather user feedback** on search quality

### 4.4 Phase 4: Optimization (Future)
1. Analyze slow queries and optimize
2. Consider caching strategies if needed
3. Fine-tune embedding model or prompts
4. Add filtering/faceting by type (atom vs triple)

## 5. Success Criteria

### 5.1 Functional Success
- [ ] Triples appear in `search_term` results
- [ ] Search for relationship concepts returns relevant triples
- [ ] New triples automatically indexed within 1 minute
- [ ] Updated triples reflected in search within 1 minute
- [ ] No degradation to existing atom search

### 5.2 Technical Success
- [ ] All existing triples backfilled into `term_text`
- [ ] Triggers execute without errors
- [ ] Embeddings generated for 100% of triple entries
- [ ] `search_term` function handles mixed results correctly
- [ ] No breaking changes to GraphQL API

### 5.3 Quality Metrics
- Search relevance: User testing/feedback
- Coverage: % of triples successfully indexed
- Freshness: Time from triple creation to searchability
- Performance: p95 search latency < 1s

## 6. Risks & Mitigations

### 6.1 Data Volume Risk
**Risk**: Large number of triples could impact embedding generation and search performance

**Mitigation**: 
- Monitor pgai queue depth and processing rate
- Consider batching backfill migration
- Add indexes on `term_text` if needed

### 6.2 Atom Label Update Risk
**Risk**: When atom labels change, triple text in `term_text` becomes stale

**Mitigation**:
- Document limitation in Phase 1
- Phase 2: Add triggers on atom table to cascade updates
- Consider denormalization trade-offs

### 6.3 Embedding Quality Risk
**Risk**: Triple text format may not produce high-quality embeddings for relationship search

**Mitigation**:
- Test search quality with sample queries
- Consider alternative text formats (e.g., more natural language)
- May need prompt engineering or fine-tuning

### 6.4 Backward Compatibility Risk
**Risk**: Changes to `search_term` could break existing clients

**Mitigation**:
- No schema changes to search_term function
- Additive only (new results, same format)
- Frontend already handles different entity types

## 7. Open Questions

1. **Atom Label Cascading**: Should atom label updates trigger triple text regeneration? If so, what's the trigger design?
2. **Type Discriminator**: Is `type` column already in `term_text` or does it need to be added?
3. **Embedding Model**: Current OpenAI model being used? Is it optimal for relationship text?
4. **Search Ranking**: Should triples be weighted differently than atoms in search results?
5. **Deletion Handling**: Do we need DELETE triggers to remove triples from `term_text`?

## 8. Future Enhancements

### 8.1 Advanced Filtering
- Filter search by entity type (atom only, triple only, both)
- Filter by predicate type (e.g., only "knows" relationships)
- Date range filtering for triples

### 8.2 Relationship-Specific Search
- Search within specific relationship types
- Graph traversal combined with semantic search
- Multi-hop relationship queries

### 8.3 Search Analytics
- Track which triple types are most searched
- Identify gaps in triple coverage
- A/B test different text formats

### 8.4 Enhanced Text Generation
- Use LLM to generate more natural triple descriptions
- Include context from related triples
- Support multiple languages

## 9. Dependencies

### 9.1 Infrastructure
- PostgreSQL with pgai extension (already deployed)
- OpenAI API access (already configured)
- Hasura GraphQL engine (already running)

### 9.2 Schema Dependencies
- Triples table with denormalized subject/predicate/object labels
- term_text table with embedding column
- search_term function implementation

### 9.3 Tooling
- Hasura migrations for trigger deployment
- Database migration scripts for backfill

## 10. Documentation Requirements

1. **Database Schema Documentation**: Update docs to reflect triple inclusion in `term_text`
2. **Trigger Documentation**: Document trigger logic and maintenance procedures
3. **API Documentation**: Update GraphQL schema docs to note triple results
4. **Migration Guide**: Document backfill process for future environments
5. **Troubleshooting Guide**: Common issues with embedding generation and search

## 11. Rollout Plan

### 11.1 Development Environment
1. Deploy triggers
2. Run backfill
3. Test search functionality
4. Validate results

### 11.2 Staging Environment
1. Deploy to staging
2. Run full backfill with production data snapshot
3. Performance testing
4. User acceptance testing

### 11.3 Production Environment
1. Deploy triggers during maintenance window
2. Run backfill migration (potentially over multiple hours)
3. Monitor embedding generation
4. Gradual rollout with feature flag (if possible)
5. Monitor performance and error rates

### 11.4 Rollback Plan
- Keep triggers but stop backfill if issues arise
- Remove triple entries from `term_text` if search quality degrades
- Disable triggers and revert migration if critical issues

---

**Document Version**: 1.0  
**Last Updated**: 2026-01-13  
**Owner**: Engineering Team  
**Status**: Draft - Pending Review