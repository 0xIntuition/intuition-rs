# Nested Triples Design Document

## Executive Summary

This document analyzes the requirements and implications of supporting **nested triples** in the Intuition system: allowing a triple's subject, predicate, or object to itself be a triple (recursively, at any depth).

---

## 1. Current Architecture Snapshot

### How triples work today

A triple is `(subject_id, predicate_id, object_id)` where **all three are atom term_ids**.

```
Triple(term_id) ──subject_id──► Atom(term_id)
                ──predicate_id──► Atom(term_id)
                ──object_id──► Atom(term_id)
```

The `term` table is the unified registry (`1729947331633_consolidated_basic_structure/up.sql:92-101`):
- `term.id` TEXT PRIMARY KEY (the term’s identity; for Atom/Triple terms this equals `atom.term_id` / `triple.term_id`)
- `term.type` = `term_type` enum: `'Atom' | 'Triple' | 'CounterTriple'`
- `term.atom_id` references `atom.term_id` (for Atom type)
- `term.triple_id` references `triple.term_id` (for Triple/CounterTriple type)

### Smart contract (on-chain)

The contract function `createTriples(bytes32[] subjectIds, bytes32[] predicateIds, bytes32[] objectIds, uint256[] assets)` takes **bytes32 IDs** for all three components. The contract does NOT enforce that these must be atom IDs — it operates purely on term IDs (bytes32). This means:

- The on-chain contract likely **already supports** nested triples at the ABI level (subject/predicate/object can be any term ID, including a triple's term ID)
- `isTriple(bytes32 termId)` and `isAtom(bytes32 termId)` exist as read functions
- `calculateTripleId(subjectId, predicateId, objectId)` is a pure function on bytes32

### What breaks today if a triple ID is passed as subject/predicate/object

1. **Event handler** (`apps/consumer/src/mode/decoded/triple_created/event.rs:58-75`): `get_subject_predicate_object_atoms()` calls `Atom::find_subject_predicate_object()` which queries only the `atom` table — returns `ModelError::QueryError("Expected 3 atoms, found N. Missing atoms for IDs: subject=..., predicate=..., object=...")` if any component is a triple (see `apps/models/src/atom.rs:336-344`).
2. **Hasura relationships** (`infrastructure/hasura/metadata/databases/intuition/tables/public_triple.yaml:28-53`): `subject`, `predicate`, `object` use `column_mapping: subject_id: term_id` with `remote_table: atom` — join on `atom.term_id` returns no row for a triple component, so the relationship resolves to null.
3. **Triple term_text trigger** (`insert_triple_term_text` in `1768319332000_add_triple_term_text_trigger/up.sql`): `SELECT ... FROM atom WHERE term_id IN (NEW.subject_id, ...)` — only atoms are matched; triple components yield NULL/empty labels.
4. **Atom label-update trigger** (`update_term_text_function` in `1768475000001_update_term_text_function_for_atom_labels/up.sql`): When an atom's label changes, it updates `term_text` for affected triples via `LEFT JOIN atom` on subject/predicate/object — any component that is a triple has no matching atom row, so the recomputed title gets NULL for that slot.
5. **predicate_object / subject_predicate triggers** (`1729947331635_consolidated_triggers/up.sql`): `update_predicate_object_on_triple_insert` and `update_subject_predicate_on_triple_insert` only INSERT `NEW.predicate_id`, `NEW.object_id` etc. — no FK or type check. They work as-is with triple IDs; aggregates will simply key by (predicate_id, object_id) even when those are triple term_ids.
6. **Account update logic** (`is_account_with_person_or_org` in `event.rs:124-144`): Expects `Atom` args and checks `AtomType`. With nested triples this is never reached because `get_subject_predicate_object_atoms()` fails first (see 1).
7. **SQL functions** (`search_positions_on_subject`, `accounts_that_claim_about_account`, `following` in `1729947331635_consolidated_triggers/up.sql`): `search_positions_on_subject` JOINs `atom` for predicate and object — triples as predicate/object won’t match. `accounts_that_claim_about_account` does `JOIN account ON account.atom_id = triple.object_id` — assumes object is an atom; triples as object are excluded. `following()` uses the same pattern.

---

## 2. Critical Design Questions

### Q1: Does the smart contract already support nested triples, or does it need changes?

**Analysis**: The `createTriples` function accepts `bytes32[]` for all IDs. If the on-chain validation only checks that `subjectId != 0 && predicateId != 0 && objectId != 0` and that these term IDs exist, then nested triples already work on-chain. The `TripleCreated` event emits `subjectId`, `predicateId`, `objectId` as `bytes32` — the indexer just needs to handle them.

**Decision needed**: Confirm with smart contract team whether `createTriples` validates that subject/predicate/object are atom IDs, or if any existing term ID is accepted.

### Q2: Should the database schema use a polymorphic reference or separate columns?

**Option A — Polymorphic via `term` table (Recommended)**:
Triple's `subject_id`, `predicate_id`, `object_id` reference `term.id` instead of `atom.term_id`. The `term` table already distinguishes type via `term.type`. Hasura relationships point to `term`, and from `term` you navigate to either `atom` or `triple`.

```
Triple ──subject_id──► Term ──atom_id──► Atom
                                ──triple_id──► Triple (recursion)
```

**Option B — Separate columns for atom vs triple references**:
Add `subject_triple_id`, `predicate_triple_id`, `object_triple_id` nullable columns alongside existing atom ID columns. This is messy and multiplicative.

**Option C — Keep current columns, change only Hasura relationships**:
Keep `subject_id`, `predicate_id`, `object_id` as TEXT (they already are), but change Hasura to resolve them through `term` instead of directly to `atom`.

**Recommendation**: Option C is the least disruptive. The column data doesn't change — it already stores term_ids. Only the interpretation layer (Hasura relationships, Rust event handler, triggers) needs updating.

### Q3: How deep can nesting go? Is there a practical limit?

**Analysis**: In theory, nesting is unbounded. In practice:
- GraphQL queries would need to specify depth explicitly (Hasura doesn't support infinite recursion)
- Label generation for `term_text` needs a recursive approach
- Performance: each level of nesting adds a JOIN

**Recommendation**: Support arbitrary depth at the data model level. Apply practical limits at the query level (e.g., max 5 levels in a single GraphQL query). Provide a `flatten_triple_label(term_id)` SQL function that recursively builds the label.

### Q4: What happens to `predicate_object` and `subject_predicate` aggregate tables?

**Current**: Triggers `update_predicate_object_on_triple_insert` and `update_subject_predicate_on_triple_insert` (`1729947331635_consolidated_triggers/up.sql:509-538`) INSERT `(NEW.predicate_id, NEW.object_id, ...)` and `(NEW.subject_id, NEW.predicate_id, ...)` with no type validation.

**With nesting**: A predicate/object/subject could be a triple term_id. The tables use TEXT keys and ON CONFLICT (predicate_id, object_id) DO UPDATE — no FK to atom.

**Recommendation**: No change. Aggregates work as-is; they will track (subject_id, predicate_id) and (predicate_id, object_id) by term_id regardless of type. The aggregate *update* triggers (`update_predicate_object_aggregates`, `update_subject_predicate_aggregates`) join `triple` and `triple_term` by these IDs and don’t touch the atom table.

### Q5: How should the `term_text` / search system handle nested triples?

**Current**: Triple label = `CONCAT(subject.label, ' ', predicate.label, ' ', object.label)` where labels come from atom table.

**With nesting**: A component's label might itself be a concatenated triple label like "Alice isFollowing Bob".

**Recommendation**: Create a recursive SQL function `get_term_label(p_term_id TEXT, p_depth INTEGER DEFAULT 0)` that:
1. If `p_depth > 10` return `'[max depth]'` (cycle / depth guard).
2. Look up `term.type` for `p_term_id`.
3. If `Atom`, return `atom.label` from `atom` where `term_id = p_term_id`.
4. If `Triple` or `CounterTriple`, select `subject_id`, `predicate_id`, `object_id` from `triple` where `term_id = p_term_id`, then return `TRIM(CONCAT(get_term_label(subject_id, p_depth+1), ' ', get_term_label(predicate_id, p_depth+1), ' ', get_term_label(object_id, p_depth+1)))`.
5. Otherwise return NULL.

### Q6: How should the Rust event handler change?

**Current flow** (`apps/consumer/src/mode/decoded/triple_created/event.rs`):
1. `get_subject_predicate_object_atoms()` → calls `Atom::find_subject_predicate_object()` (single query to `atom` for all 3 IDs); errors if not exactly 3 atoms.
2. `check_and_update_account_predicate_object()` uses that tuple; `is_account_with_person_or_org()` checks `AtomType` on all 3; `update_account()` updates account from atom data.

**Required change**: Instead of querying only the `atom` table, query the `term` table first to determine what type each component is, then fetch the appropriate entity.

**Recommendation**:
```rust
// New approach
async fn get_subject_predicate_object_terms(
    &self,
    context: &DecodedConsumerContext,
) -> Result<(TermInfo, TermInfo, TermInfo), ConsumerError>
```
Where `TermInfo` is an enum:
```rust
enum TermInfo {
    Atom(Atom),
    Triple(Triple),
}
```

The `check_and_update_account_predicate_object` logic only applies when all three components are atoms, so it can simply skip when any component is a triple.

### Q7: What about the counter-triple for a nested triple?

**Analysis**: Counter-triples are derived deterministically: `counter_id = keccak256(COUNTER_SALT || triple_id)`. This is independent of whether components are atoms or triples. No change needed.

### Q8: Does the vault/economic system need changes?

**Analysis**: Vaults are associated with term_ids. A triple that has another triple as its subject still gets its own vault via its own term_id. The vault system is term-id-based, not component-type-based.

**Answer**: No changes needed. The economic layer is already polymorphic through the `term` table.

### Q9: Ordering constraint — can a triple reference a triple that hasn't been created yet?

**Analysis**: On-chain, atoms are created first, then triples reference them. For nested triples, the inner triple MUST exist before the outer triple that references it. The blockchain event ordering (block_number, log_index) guarantees this.

**In the indexer**: The event handler processes events in order. If a `TripleCreated` event references another triple's term_id as subject, that referenced triple should already be in the database (from an earlier event).

**Edge case**: What if events arrive out of order in the Redis stream? The current retry-with-backoff logic (`retry_with_backoff`) handles this for atoms — it retries until the referenced atoms exist. The same pattern should work for referenced triples.

### Q10: How do GraphQL queries look for nested triples?

**Current**:
```graphql
{
  triples {
    subject { label atom_type }    # Always an atom
    predicate { label atom_type }  # Always an atom
    object { label atom_type }     # Always an atom
  }
}
```

**With nesting — Option A (via term table)**:
```graphql
{
  triples {
    subject_term {
      type       # "Atom" or "Triple"
      atom { label atom_type }
      triple {
        subject_term { ... }
        predicate_term { ... }
        object_term { ... }
      }
    }
  }
}
```

**With nesting — Option B (union/discriminated)**:
Not natively supported by Hasura without computed fields or custom resolvers.

**Recommendation**: Add new relationships (`subject_term`, `predicate_term`, `object_term`) pointing to `term`, while keeping the existing `subject`, `predicate`, `object` relationships for backward compatibility (they'll return null for triple components). This is a non-breaking change.

---

## 3. Implementation Plan

### Phase 1: Database Schema Migration (non-breaking)

**New migration file**:

```sql
-- 1. Add new relationships: triple components → term table
-- (No schema change needed — subject_id/predicate_id/object_id already store term_ids)

-- 2. Create recursive label function
CREATE OR REPLACE FUNCTION get_term_label(p_term_id TEXT, p_depth INTEGER DEFAULT 0)
RETURNS TEXT
LANGUAGE plpgsql STABLE AS $$
DECLARE
    term_record RECORD;
    result TEXT;
    sub_label TEXT;
    pred_label TEXT;
    obj_label TEXT;
BEGIN
    -- Prevent infinite recursion
    IF p_depth > 10 THEN
        RETURN '[max depth]';
    END IF;

    -- Look up the term type
    SELECT type, atom_id, triple_id INTO term_record
    FROM term WHERE id = p_term_id;

    IF NOT FOUND THEN
        RETURN NULL;
    END IF;

    IF term_record.type = 'Atom' THEN
        SELECT label INTO result FROM atom WHERE term_id = p_term_id;
        RETURN result;
    ELSIF term_record.type IN ('Triple', 'CounterTriple') THEN
        SELECT
            get_term_label(t.subject_id, p_depth + 1),
            get_term_label(t.predicate_id, p_depth + 1),
            get_term_label(t.object_id, p_depth + 1)
        INTO sub_label, pred_label, obj_label
        FROM triple t WHERE t.term_id = p_term_id;

        RETURN TRIM(CONCAT(
            COALESCE(sub_label, ''), ' ',
            COALESCE(pred_label, ''), ' ',
            COALESCE(obj_label, '')
        ));
    END IF;

    RETURN NULL;
END;
$$;

-- 3. Update the triple term_text trigger to use recursive labels
CREATE OR REPLACE FUNCTION insert_triple_term_text()
RETURNS TRIGGER AS $$
DECLARE
    subject_label TEXT;
    predicate_label TEXT;
    object_label TEXT;
BEGIN
    subject_label := get_term_label(NEW.subject_id);
    predicate_label := get_term_label(NEW.predicate_id);
    object_label := get_term_label(NEW.object_id);
    INSERT INTO term_text (id, title, description, type)
    VALUES (
        NEW.term_id,
        TRIM(CONCAT(COALESCE(subject_label, ''), ' ', COALESCE(predicate_label, ''), ' ', COALESCE(object_label, ''))),
        ';', 'triple'
    )
    ON CONFLICT (id) DO UPDATE SET title = EXCLUDED.title;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 4. Update the atom label-change trigger (update_term_text_function from 1768475000001):
-- In the atom UPDATE branch, replace the LEFT JOIN atom subject/predicate/object with
-- SET title = TRIM(CONCAT(get_term_label(t.subject_id), ' ', get_term_label(t.predicate_id), ' ', get_term_label(t.object_id)))
-- so triples with nested components get correct titles when an atom label changes.
```

### Phase 2: Hasura Metadata Updates

**Add new relationships to `public_triple.yaml`** (alongside existing ones for backward compat):

```yaml
# New polymorphic relationships via term table
- name: subject_term
  using:
    manual_configuration:
      column_mapping:
        subject_id: id
      remote_table:
        name: term
        schema: public

- name: predicate_term
  using:
    manual_configuration:
      column_mapping:
        predicate_id: id
      remote_table:
        name: term
        schema: public

- name: object_term
  using:
    manual_configuration:
      column_mapping:
        object_id: id
      remote_table:
        name: term
        schema: public
```

The `term` table already has relationships to both `atom` and `triple`, so clients can navigate:
```graphql
triple → subject_term → atom (if type=Atom)
triple → subject_term → triple (if type=Triple) → subject_term → ...
```

**Verify `public_term.yaml`** has these relationships:
```yaml
- name: atom
  using:
    manual_configuration:
      column_mapping:
        atom_id: term_id
      remote_table:
        name: atom
        schema: public
- name: triple
  using:
    manual_configuration:
      column_mapping:
        triple_id: term_id
      remote_table:
        name: triple
        schema: public
```

### Phase 3: Rust Event Handler Changes

**File**: `apps/consumer/src/mode/decoded/triple_created/event.rs`

1. Replace `get_subject_predicate_object_atoms()` with a term-aware lookup:

```rust
/// Represents a resolved component of a triple (either an atom or another triple)
pub enum TripleComponent {
    Atom(Atom),
    Triple(Triple),
}

impl TripleComponent {
    pub fn as_atom(&self) -> Option<&Atom> {
        match self {
            TripleComponent::Atom(a) => Some(a),
            _ => None,
        }
    }
}
```

2. New function to resolve components:

```rust
async fn get_subject_predicate_object_terms(
    &self,
    ctx: &DecodedConsumerContext,
) -> Result<(TripleComponent, TripleComponent, TripleComponent), ConsumerError> {
    let ids = [self.subject_id()?, self.predicate_id()?, self.object_id()?];
    let mut components = Vec::with_capacity(3);

    for id in &ids {
        // Try atom first (most common case)
        if let Some(atom) = Atom::find_by_id(id.clone(), &ctx.backend_schema, &ctx.pg_pool).await? {
            components.push(TripleComponent::Atom(atom));
        } else if let Some(triple) = Triple::find_by_id(id.clone(), &ctx.backend_schema, &ctx.pg_pool).await? {
            components.push(TripleComponent::Triple(triple));
        } else {
            return Err(ConsumerError::TermNotFound(id.to_string()));
        }
    }

    Ok((
        components.remove(0),
        components.remove(0),
        components.remove(0),
    ))
}
```

3. Update `check_and_update_account_predicate_object` to only operate on atom components:

```rust
async fn check_and_update_account_predicate_object(
    &self,
    ctx: &DecodedConsumerContext,
) -> Result<(), ConsumerError> {
    let (subject, predicate, object) = self
        .get_subject_predicate_object_terms(ctx)
        .await?;

    // Account-predicate-object logic only applies to atom components
    if let (
        TripleComponent::Atom(subject_atom),
        TripleComponent::Atom(predicate_atom),
        TripleComponent::Atom(object_atom),
    ) = (&subject, &predicate, &object)
    {
        if self.is_account_with_person_or_org(subject_atom, predicate_atom, object_atom) {
            self.update_account(ctx, subject_atom, object_atom).await?;
        }
    }
    Ok(())
}
```

### Phase 4: Model Changes

**File**: `apps/models/src/triple.rs`

The `Triple` struct itself doesn't need changes — `subject_id`, `predicate_id`, `object_id` already store `FixedBytesWrapper` (32-byte term IDs) that work for both atoms and triples.

However, `Atom::find_subject_predicate_object` (used by the event handler) needs an alternative. Add a `Term`-based lookup:

```rust
// In apps/models/src/term.rs
impl Term {
    pub async fn find_by_ids<'e, E>(
        ids: &[FixedBytesWrapper],
        schema: &str,
        executor: E,
    ) -> Result<Vec<Self>, ModelError>
    where
        E: Executor<'e, Database = Postgres>,
    {
        // Query term table for multiple IDs at once
        // Returns type information to determine atom vs triple
    }
}
```

---

## 4. Impact Analysis

### What does NOT need to change

| Component | Reason |
|-----------|--------|
| `triple` table schema | `subject_id`/`predicate_id`/`object_id` are TEXT, already accept any term_id |
| `term` table | Already supports Atom/Triple/CounterTriple types |
| Vault system | Operates on term_ids, type-agnostic |
| Position system | References term_ids only |
| Counter-triple derivation | Hash-based, content-agnostic |
| Deposit/Redemption events | Reference term_ids only |
| `predicate_object` / `subject_predicate` triggers | INSERT uses NEW.predicate_id/object_id/subject_id only; no type validation (works with triple IDs) |
| Stats triggers | Simple counters, type-agnostic |
| Share price tracking | References term_ids only |

### What DOES need to change

| Component | Change | Risk |
|-----------|--------|------|
| `TripleCreatedEvent` trait (`apps/consumer/.../triple_created/event.rs`) | Replace `get_subject_predicate_object_atoms` with term-aware lookup | Medium — core event processing path |
| Hasura triple relationships (`public_triple.yaml`) | Add `subject_term`, `predicate_term`, `object_term` alongside existing | Low — additive change |
| `insert_triple_term_text` trigger | Use recursive `get_term_label()` instead of `FROM atom WHERE term_id IN (...)` | Low — isolated change |
| `update_term_text_function` (atom label branch) | Recompute triple titles via `get_term_label(t.subject_id)` etc. instead of LEFT JOIN atom | Low — same migration |
| GraphQL consumers (frontends) | Update queries to use new `*_term` relationships for nested data | Medium — client changes |
| `search_positions_on_subject` | JOINs `atom` for predicate/object — triples as pred/obj won’t match; update or document exclusion | Low |
| `accounts_that_claim_about_account`, `following()` | Assume `triple.object_id` is atom (account.atom_id = object_id); triples as object excluded; document or extend | Low |

### Backward Compatibility

- Existing `subject`, `predicate`, `object` Hasura relationships → kept, will return null for triple components
- Existing GraphQL queries → continue working for atom-only triples
- New `subject_term`, `predicate_term`, `object_term` relationships → additive, opt-in
- Database migration → purely additive (new function, updated trigger)

---

## 5. Open Questions for Team Discussion

1. **Smart contract validation**: Does `createTriples` on-chain validate that subject/predicate/object are atom IDs? Or does it accept any existing term ID? This determines if nested triples are already possible on-chain or require a contract upgrade.

2. **Should predicates be allowed to be triples?** Semantically, a predicate is usually a simple relationship type ("isFollowing", "likes"). Allowing a triple-as-predicate is technically possible but may be semantically confusing. Should we restrict this?

3. **Counter-triples of nested triples**: If triple T1 = (T2, atom_pred, atom_obj) where T2 is itself a triple, the counter-triple of T1 negates the outer claim. Is this the desired semantic? Or should negation propagate inward?

4. **Depth limit on-chain vs off-chain**: Should we enforce a maximum nesting depth in the indexer, even if the contract allows deeper? This affects query performance and complexity.

5. **Frontend readiness**: Are frontend clients prepared to handle recursive GraphQL queries? What's the expected UX for displaying a nested triple?

6. **Reindexing**: Will we need to reprocess historical events, or are nested triples only expected going forward?

7. **Term text search**: For deeply nested triples, the concatenated label could become very long. Should we truncate or summarize?

8. **Following / accounts_that_claim_about_account**: These functions assume `triple.object_id` is an atom (join `account ON account.atom_id = triple.object_id`). For nested triples with a triple as object, such positions are excluded. Is that acceptable, or should we add a variant that resolves via `term`?

---

## 6. Migration Strategy

### Recommended rollout order:

1. **Confirm on-chain support** (Q1 above)
2. **Database migration**: Add `get_term_label` function, update `insert_triple_term_text` trigger
3. **Hasura metadata**: Add new `*_term` relationships (backward compatible)
4. **Rust event handler**: Update to term-aware component lookup
5. **Frontend**: Gradually adopt new `*_term` relationships
6. **Testing**: Create nested triples on testnet, verify end-to-end flow
7. **Cleanup**: Deprecate old `subject`/`predicate`/`object` direct-to-atom relationships once frontends have migrated

### Test cases:

- Triple where subject is a triple, predicate and object are atoms
- Triple where all three components are triples
- Triple where subject is a triple whose subject is also a triple (depth 3)
- Label generation for nested triples
- Semantic search hitting nested triple labels
- Vault operations (deposit/redeem) on nested triples
- Counter-triple creation for nested triples
- Event processing order: inner triple event arrives before outer triple event

---

## 7. Verification note

This document was verified against the codebase (Feb 2026): event handler and `Atom::find_subject_predicate_object` in `apps/consumer` and `apps/models`, Hasura metadata under `infrastructure/hasura/`, migrations in `infrastructure/hasura/migrations/intuition/` (consolidated structure and triggers, triple term_text and atom label update), and `get_counter_id_from_triple_id` in `apps/consumer/src/mode/decoded/utils.rs` (keccak256(COUNTER_SALT || triple_id)). Counter-triple derivation is independent of component types; no change needed.
