# US-001: Schema Investigation Findings

## Investigation Date
2026-01-13

## Summary
This document contains the findings from investigating the current database schema to determine what changes are needed for triple search integration as outlined in the PRD.

---

## 1. term_text Table Schema

### Current Schema
Located in: `infrastructure/hasura/migrations/intuition/1729947331633_consolidated_basic_structure/up.sql:398-402`

```sql
CREATE TABLE IF NOT EXISTS term_text (
  id TEXT PRIMARY KEY NOT NULL,
  title TEXT,
  description TEXT
);
```

### Columns
- **id** (TEXT, PRIMARY KEY, NOT NULL): Term ID this text data represents
- **title** (TEXT, NULLABLE): Title or label of the term for embedding generation
- **description** (TEXT, NULLABLE): Description text used by pgai vectorizer for semantic search

### Key Finding #1: NO 'type' COLUMN EXISTS ❌
**The term_text table does NOT have a 'type' column to discriminate between atoms and triples.**

This means:
- We need to add a `type` column as described in Phase 1 of the PRD
- The suggested SQL in the PRD is correct: `ALTER TABLE term_text ADD COLUMN IF NOT EXISTS type TEXT`
- We should consider using the existing `term_type` enum: `('Atom', 'Triple', 'CounterTriple')`

### Current Population Mechanism
The table is populated via triggers on typed entity tables:
- `thing` table → `thing_insert_update_term_text_trigger`
- `person` table → `person_insert_update_term_text_trigger`
- `book` table → `book_insert_update_term_text_trigger`
- `organization` table → `organization_insert_update_term_text_trigger`

All triggers call `update_term_text_function()` (defined in `infrastructure/hasura/migrations/intuition/1729947331635_consolidated_triggers/up.sql:477-507`)

### Embedding Generation
The table is configured with pgai vectorizer in `infrastructure/hasura/migrations/intuition/1729947331636_consolidated_materialized_views_and_extensions/up.sql:18-25`:

```sql
SELECT ai.create_vectorizer(
    'term_text'::regclass,
    destination => ai.destination_table('term_embeddings'),
    embedding => ai.embedding_openai('text-embedding-3-small', 768),
    loading => ai.loading_column('description'),
    formatting => ai.formatting_python_template('title: $title id: $id $chunk')
);
```

**Key Details:**
- Embeddings stored in separate `term_embeddings` table (auto-created by pgai)
- Uses OpenAI's `text-embedding-3-small` model with 768 dimensions
- Embeddings are generated from the `description` column
- Formatting template includes both title and id for context

---

## 2. triple Table Schema

### Current Schema
Located in: `infrastructure/hasura/migrations/intuition/1729947331633_consolidated_basic_structure/up.sql:122-132`

```sql
CREATE TABLE IF NOT EXISTS triple (
  term_id TEXT PRIMARY KEY NOT NULL,
  creator_id TEXT NOT NULL,
  subject_id TEXT NOT NULL,
  predicate_id TEXT NOT NULL,
  object_id TEXT NOT NULL,
  counter_term_id TEXT NOT NULL,
  block_number NUMERIC(78, 0) NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL,
  transaction_hash TEXT NOT NULL
);
```

### Columns
- **term_id** (TEXT, PRIMARY KEY, NOT NULL): Unique identifier linking to the term registry
- **creator_id** (TEXT, NOT NULL): Account that created this triple via smart contract transaction
- **subject_id** (TEXT, NOT NULL): Subject atom ID forming the first element of the triple
- **predicate_id** (TEXT, NOT NULL): Predicate atom ID forming the relationship of the triple
- **object_id** (TEXT, NOT NULL): Object atom ID forming the third element of the triple
- **counter_term_id** (TEXT, NOT NULL): Term ID of the opposing CounterTriple for this triple
- **block_number** (NUMERIC(78, 0), NOT NULL): Block number when this triple was created
- **created_at** (TIMESTAMP WITH TIME ZONE, NOT NULL): Timestamp when this triple was created
- **transaction_hash** (TEXT, NOT NULL): Transaction hash of the triple creation event

### Key Finding #2: NO DENORMALIZED TEXT FIELDS ❌
**The triple table does NOT have denormalized `subject_label`, `predicate_label`, or `object_label` columns.**

This contradicts the assumption in the PRD (Section 2.1.2):
> "Triples already have denormalized text fields for subject/predicate/object labels"

**Current Reality:**
- Triple table only stores IDs (`subject_id`, `predicate_id`, `object_id`)
- Labels must be retrieved via JOINs to the `atom` table
- This means triggers WILL require joins during execution

### Hasura Relationships
The triple table has Hasura relationships defined in `infrastructure/hasura/metadata/databases/intuition/tables/public_triple.yaml`:
- **subject** → joins to `atom` table via `subject_id`
- **predicate** → joins to `atom` table via `predicate_id`
- **object** → joins to `atom` table via `object_id`

These are GraphQL relationships, not database foreign keys.

---

## 3. search_term Function

### Function Signature
Located in: `infrastructure/hasura/migrations/intuition/1729947331636_consolidated_materialized_views_and_extensions/up.sql:31-41`

```sql
CREATE FUNCTION search_term (query text)
RETURNS SETOF term
LANGUAGE sql STABLE
AS $$
    SELECT t.id, t.type, t.atom_id, t.triple_id, t.total_assets, t.total_market_cap, t.created_at, t.updated_at
    FROM (
        SELECT
            t.id,
            embedding <=> ai.openai_embed('text-embedding-3-small', query, dimensions=>768) as distance
        FROM term_embeddings
        LEFT JOIN term t ON term_embeddings.id = t.id
        ORDER BY distance
    ) s
    JOIN term t ON s.id = t.id
$$;
```

### Behavior
- **Input**: `query` (text) - natural language search query
- **Output**: SETOF `term` - returns rows from the `term` table
- **Ranking**: Results ordered by cosine distance (lower = more similar)
- **Embedding**: Query is embedded in real-time using OpenAI API
- **Join**: Results joined back to `term` table to get full term details

### Return Columns
Returns all columns from the `term` table:
- `id` (TEXT)
- `type` (term_type enum: 'Atom', 'Triple', 'CounterTriple')
- `atom_id` (TEXT, nullable)
- `triple_id` (TEXT, nullable)
- `total_assets` (NUMERIC(78, 0))
- `total_market_cap` (NUMERIC(78, 0))
- `created_at` (TIMESTAMP WITH TIME ZONE)
- `updated_at` (TIMESTAMP WITH TIME ZONE)

### Key Finding #3: Function Already Supports Mixed Results ✅
**The search_term function is polymorphic and ALREADY returns the `type` field, which will allow differentiation between atoms and triples.**

No changes needed to the function itself - it will automatically work once triples are added to `term_text`.

---

## 4. Related Finding: atom Table and Labels

### atom Table Schema
Located in: `infrastructure/hasura/migrations/intuition/1729947331633_consolidated_basic_structure/up.sql:103-120`

The `atom` table has:
- **label** (TEXT, NULLABLE): Human-readable label or title for this atom
- **emoji** (TEXT, NULLABLE): Optional emoji representation

The `label` column is what we need to concatenate for triple text generation.

### atom Rust Model
Located in: `apps/models/src/atom.rs`

The Rust model confirms the `label` field exists and is used throughout the application.

---

## 5. Critical Design Decision Required

### The Denormalization Problem
The PRD assumes denormalized label fields exist in the `triple` table, but they don't. This creates two architectural options:

#### Option A: Add Denormalized Columns to triple Table
**Pros:**
- Trigger performance (no JOINs during INSERT)
- Matches PRD assumption
- Simple trigger logic

**Cons:**
- Data duplication (labels stored in both `atom` and `triple`)
- Requires migration to backfill existing triples
- Must handle atom label updates (cascade updates to all referencing triples)
- More complex atom update triggers

#### Option B: Use JOINs in Triggers (No Denormalization)
**Pros:**
- No data duplication
- Single source of truth for labels
- Atom label updates automatically reflected (via re-trigger)
- No additional storage overhead

**Cons:**
- Trigger performance (3 JOINs per triple INSERT)
- More complex trigger SQL
- Potential for join failures if atoms don't exist yet

### Recommendation: Option B (JOIN-based triggers)
**Rationale:**
1. PostgreSQL is optimized for JOINs, and we're joining on indexed PKs
2. Atom labels rarely change, so cascade update complexity isn't worth it
3. Avoids data consistency issues
4. Triple creation is not on the critical path (async processing)
5. Simpler long-term maintenance (single source of truth)

### Atom Label Update Strategy
If we use Option B, we need a trigger on the `atom` table to update `term_text` when atom labels change:

```sql
CREATE OR REPLACE FUNCTION update_triple_text_on_atom_change()
RETURNS TRIGGER AS $$
BEGIN
    -- Update term_text for all triples where this atom is subject, predicate, or object
    UPDATE term_text tt
    SET
        title = (
            SELECT CONCAT(
                COALESCE(s.label, ''), ' ',
                COALESCE(p.label, ''), ' ',
                COALESCE(o.label, '')
            )
            FROM triple t
            LEFT JOIN atom s ON t.subject_id = s.term_id
            LEFT JOIN atom p ON t.predicate_id = p.term_id
            LEFT JOIN atom o ON t.object_id = o.term_id
            WHERE t.term_id = tt.id
        ),
        description = (/* same as title for now */)
    WHERE id IN (
        SELECT term_id FROM triple
        WHERE subject_id = NEW.term_id
           OR predicate_id = NEW.term_id
           OR object_id = NEW.term_id
    );

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
```

---

## 6. Implementation Impact on PRD

### Changes Needed to PRD Section 2.1.2
The statement "Triples already have denormalized text fields" is **incorrect**.

Update to:
> "Triples store only IDs for subject/predicate/object. Labels must be retrieved via JOINs to the atom table during trigger execution. This requires carefully crafted trigger functions with LEFT JOINs to the atom table."

### Changes Needed to PRD Section 2.2
All pseudo-code examples need to be updated to include JOINs:

**Correct INSERT Trigger Pseudo-code:**
```sql
ON INSERT INTO triple
  INSERT INTO term_text (id, title, description)
  SELECT
    NEW.term_id,
    CONCAT(s.label, ' ', p.label, ' ', o.label),
    CONCAT(s.label, ' ', p.label, ' ', o.label)
  FROM triple t
  LEFT JOIN atom s ON NEW.subject_id = s.term_id
  LEFT JOIN atom p ON NEW.predicate_id = p.term_id
  LEFT JOIN atom o ON NEW.object_id = o.term_id
  WHERE t.term_id = NEW.term_id
```

### New Section Needed in PRD
Add a new section "2.2.4 Atom Label Update Triggers" documenting the cascade update strategy.

---

## 7. Open Questions for Next Steps

1. **type Column Type**: Should we use TEXT or the existing `term_type` enum?
   - Recommendation: Use `term_type` enum for type safety and consistency

2. **NULL Label Handling**: How should we handle atoms with NULL labels?
   - Recommendation: Use `COALESCE(label, '')` to avoid NULL concatenation issues

3. **Counter-Triple Handling**: Should counter-triples also be indexed?
   - PRD mentions 'Triple' and 'CounterTriple' as separate types
   - Need to clarify if counter-triples should have separate term_text entries

4. **Backfill Performance**: Estimate the number of existing triples to backfill
   - Need to query production database for count
   - May need batching strategy if count is very high

5. **Embedding Quality**: Should title and description be different?
   - Current atom triggers use `name` → `title` and `description` → `description`
   - For triples, we only have concatenated labels
   - Recommendation: Use same text for both initially, evaluate quality

---

## 8. Summary of Acceptance Criteria Status

- [x] **Document current term_text table schema including all columns**
  - ✅ Documented in Section 1

- [x] **Verify if 'type' column exists in term_text**
  - ✅ DOES NOT EXIST - needs to be added (Section 1, Key Finding #1)

- [x] **Document triples table schema with subject_label, predicate_label, object_label fields**
  - ✅ Documented in Section 2 - **FIELDS DO NOT EXIST** (Key Finding #2)

- [x] **Confirm triples have denormalized text fields (not just IDs)**
  - ✅ CONFIRMED: Triples DO NOT have denormalized text fields (Section 2, Key Finding #2)

- [x] **Document current search_term function signature and behavior**
  - ✅ Documented in Section 3 - function already supports mixed results (Key Finding #3)

---

## 9. Next Steps

Based on these findings, the next user stories should be:

1. **US-002**: Add `type` column to `term_text` table with migration
2. **US-003**: Create triple INSERT trigger with JOINs to populate `term_text`
3. **US-004**: Create atom UPDATE trigger to cascade label changes to triples
4. **US-005**: Create backfill migration script for existing triples
5. **US-006**: Test and validate search results with mixed atom/triple data

---

## 10. Files Referenced

### Schema Definitions
- `infrastructure/hasura/migrations/intuition/1729947331633_consolidated_basic_structure/up.sql`

### Triggers and Functions
- `infrastructure/hasura/migrations/intuition/1729947331635_consolidated_triggers/up.sql`
- `infrastructure/hasura/migrations/intuition/1729947331636_consolidated_materialized_views_and_extensions/up.sql`

### Metadata
- `infrastructure/hasura/metadata/databases/intuition/tables/public_triple.yaml`

### Models
- `apps/models/src/triple.rs`
- `apps/models/src/atom.rs`

---

**Investigation Complete** ✅
