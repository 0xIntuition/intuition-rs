# Foreign Key Relationships Map

## Core Entity Hierarchy

```
term (id) [ROOT TABLE]
├── atom (term_id) → term(id)
│   ├── account (atom_id) → atom(term_id) [nullable]
│   ├── triple (subject_id) → atom(term_id)
│   ├── triple (predicate_id) → atom(term_id)
│   ├── triple (object_id) → atom(term_id)
│   ├── predicate_object (predicate_id) → atom(term_id)
│   ├── predicate_object (object_id) → atom(term_id)
│   ├── subject_predicate (subject_id) → atom(term_id)
│   ├── subject_predicate (predicate_id) → atom(term_id)
│   └── atom_value (id) → atom(term_id)
├── triple (term_id) → term(id)
├── vault (term_id) → term(id) [composite PK with curve_id]
├── deposit (term_id) → term(id)
├── redemption (term_id) → term(id)
├── position (term_id) → term(id)
├── signal (term_id) → term(id)
├── share_price_change (term_id) → term(id)
├── thing (id) → term(id)
├── person (id) → term(id)
├── organization (id) → term(id)
├── book (id) → term(id)
├── caip10 (id) → term(id)
├── json_object (id) → term(id)
├── text_object (id) → term(id)
└── byte_object (id) → term(id)
```

## Account Relationships

```
account (id) [ROOT TABLE]
├── atom (wallet_id) → account(id)
├── atom (creator_id) → account(id)
├── triple (creator_id) → account(id)
├── deposit (sender_id) → account(id)
├── deposit (receiver_id) → account(id)
├── redemption (sender_id) → account(id)
├── redemption (receiver_id) → account(id)
├── fee_transfer (sender_id) → account(id)
├── fee_transfer (receiver_id) → account(id)
├── position (account_id) → account(id)
├── signal (account_id) → account(id)
└── atom_value (account_id) → account(id) [nullable]
```

## Vault System (Composite Keys)

```
vault (term_id, curve_id) [COMPOSITE PRIMARY KEY]
├── deposit (term_id, curve_id) → vault(term_id, curve_id)
├── redemption (term_id, curve_id) → vault(term_id, curve_id)
├── position (term_id, curve_id) → vault(term_id, curve_id)
└── signal (term_id, curve_id) → vault(term_id, curve_id)
```

## Transaction Chain Relationships

```
deposit (id) [ROOT TABLE]
├── event (deposit_id) → deposit(id) [nullable]
└── signal (deposit_id) → deposit(id) [nullable]

redemption (id) [ROOT TABLE]
├── event (redemption_id) → redemption(id) [nullable]
└── signal (redemption_id) → redemption(id) [nullable]

fee_transfer (id) [ROOT TABLE]
└── event (fee_transfer_id) → fee_transfer(id) [nullable]
```

## Value Object Relationships

```
atom_value (id) [PRIMARY KEY]
├── thing (id) → atom_value(id) [via term(id)]
├── person (id) → atom_value(id) [via term(id)]
├── organization (id) → atom_value(id) [via term(id)]
├── book (id) → atom_value(id) [via term(id)]
├── caip10 (id) → atom_value(id) [via term(id)]
├── json_object (id) → atom_value(id) [via term(id)]
├── text_object (id) → atom_value(id) [via term(id)]
└── byte_object (id) → atom_value(id) [via term(id)]
```

## Potential Issues Identified

### 1. Circular Dependencies
- `account` → `atom` → `account` (via atom_id)
- This creates a potential circular reference

### 2. Complex Composite Key Dependencies
- Multiple tables depend on `vault(term_id, curve_id)` composite key
- Any changes to vault structure affect 4 other tables

### 3. Term Table as Central Hub
- `term` table is referenced by 10+ tables
- Single point of failure for many operations

### 4. Nullable Foreign Keys
- Several nullable FKs could lead to orphaned records
- `account(atom_id)`, `signal(deposit_id)`, `signal(redemption_id)`, etc.

### 5. Missing Constraints
- No foreign key from `triple_vault` to `vault` table
- `triple_term` table has no foreign key constraints defined
