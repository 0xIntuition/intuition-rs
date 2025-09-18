# Atoms Query Guide

This guide shows how to fetch atoms from the Hasura GraphQL API based on the schema configuration.

## Available Queries

### 1. Simple Atoms Query (`get-atoms-simple.graphql`)
Basic atom fields without relationships - fastest for bulk operations.

```graphql
query GetAtomsSimple($limit: Int, $offset: Int, $order_by: [atoms_order_by!], $where: atoms_bool_exp) {
  atoms(limit: $limit, offset: $offset, order_by: $order_by, where: $where) {
    term_id
    wallet_id
    creator_id
    data
    emoji
    label
    image
    type
    value_id
    block_number
    block_timestamp
    transaction_hash
  }
}
```

### 2. Detailed Atoms Query (`get-atoms-detailed.graphql`)
Atoms with core relationships (controller, creator, term, atom_value, cached_image).

```graphql
query GetAtomsDetailed($limit: Int, $offset: Int, $order_by: [atoms_order_by!], $where: atoms_bool_exp) {
  atoms(limit: $limit, offset: $offset, order_by: $order_by, where: $where) {
    term_id
    wallet_id
    creator_id
    data
    emoji
    label
    image
    type
    value_id
    block_number
    block_timestamp
    transaction_hash
    
    # Core relationships
    controller { id, address, type, created_at }
    creator { id, address, type, created_at }
    term { term_id, name, description, created_at }
    atom_value { id, data, type, created_at }
    cached_image { id, original_url, cached_url, created_at }
  }
}
```

### 3. Full Atoms Query (`get-atoms.graphql`)
Complete atoms with all relationships including triples, positions, signals.

```graphql
query GetAtoms($limit: Int, $offset: Int, $order_by: [atoms_order_by!], $where: atoms_bool_exp) {
  atoms(limit: $limit, offset: $offset, order_by: $order_by, where: $where) {
    # All basic fields
    term_id, wallet_id, creator_id, data, emoji, label, image, type
    value_id, block_number, block_timestamp, transaction_hash
    
    # All relationships with limits
    controller { id, address, type, created_at }
    creator { id, address, type, created_at }
    term { term_id, name, description, created_at }
    atom_value { id, data, type, created_at }
    cached_image { id, original_url, cached_url, created_at }
    
    # Array relationships (limited to 10 each)
    accounts(limit: 10) { id, address, type, created_at }
    positions(limit: 10) { id, term_id, amount, created_at }
    signals(limit: 10) { id, atom_id, signal_type, created_at }
    as_subject_triples(limit: 10) { term_id, subject_id, predicate_id, object_id, created_at }
    as_predicate_triples(limit: 10) { term_id, subject_id, predicate_id, object_id, created_at }
    as_object_triples(limit: 10) { term_id, subject_id, predicate_id, object_id, created_at }
  }
}
```

## Usage Examples

### Basic Usage

```rust
use crate::queries::get_atoms_simple::{GetAtomsSimple, Variables};

let variables = Variables {
    limit: Some(100),
    offset: Some(0),
    order_by: Some(vec![AtomsOrderBy {
        term_id: Some(OrderBy::Desc),
    }]),
    where_: None,
};

let request_body = GetAtomsSimple::build_query(variables);
```

### Filtering Examples

#### By Atom Type
```rust
let where_condition = AtomsBoolExp {
    type_: Some(AtomTypeComparisonExp {
        _eq: Some("Person".to_string()),
    }),
    ..Default::default()
};
```

#### By Creator Address
```rust
let where_condition = AtomsBoolExp {
    creator_id: Some(StringComparisonExp {
        _eq: Some("0x1234...".to_string()),
    }),
    ..Default::default()
};
```

#### By Block Number Range
```rust
let where_condition = AtomsBoolExp {
    block_number: Some(NumericComparisonExp {
        _gte: Some("1000000".to_string()),
        _lte: Some("2000000".to_string()),
    }),
    ..Default::default()
};
```

#### Complex Filters
```rust
let where_condition = AtomsBoolExp {
    _and: Some(vec![
        AtomsBoolExp {
            type_: Some(AtomTypeComparisonExp {
                _eq: Some("Person".to_string()),
            }),
            ..Default::default()
        },
        AtomsBoolExp {
            block_number: Some(NumericComparisonExp {
                _gte: Some("1000000".to_string()),
            }),
            ..Default::default()
        },
    ]),
    ..Default::default()
};
```

### Sorting Examples

#### By Term ID (newest first)
```rust
let order_by = vec![AtomsOrderBy {
    term_id: Some(OrderBy::Desc),
}];
```

#### By Block Number (oldest first)
```rust
let order_by = vec![AtomsOrderBy {
    block_number: Some(OrderBy::Asc),
}];
```

#### Multiple Sort Criteria
```rust
let order_by = vec![
    AtomsOrderBy {
        type_: Some(OrderBy::Asc),
    },
    AtomsOrderBy {
        block_number: Some(OrderBy::Desc),
    },
];
```

## Performance Considerations

1. **Use Simple Query for Bulk Operations**: When you only need basic atom data, use `GetAtomsSimple` for better performance.

2. **Limit Relationship Queries**: The full query includes many relationships. Use smaller limits (10-25) for complex queries.

3. **Pagination**: Always use `limit` and `offset` for large datasets. The default limit is 250.

4. **Filtering**: Use `where` clauses to reduce the dataset size before fetching relationships.

5. **Caching**: Consider caching frequently accessed atoms, especially those with complex relationships.

## Available Atom Types

Based on the schema, atoms can have these types:
- `Account`
- `ByteObject`
- `Book`
- `Caip10`
- `FollowAction`
- `JsonObject`
- `Keywords`
- `LikeAction`
- `Organization`
- `OrganizationPredicate`
- `Person`
- `PersonPredicate`
- `TextObject`
- `Thing`
- `ThingPredicate`
- `Unknown`

## Error Handling

```rust
match fetch_atoms().await {
    Ok(atoms) => {
        println!("Fetched {} atoms", atoms.len());
        for atom in atoms {
            println!("Atom: {} - {}", atom.term_id, atom.label.unwrap_or_default());
        }
    }
    Err(e) => {
        eprintln!("Error fetching atoms: {}", e);
    }
}
```

## Rate Limiting

The Hasura endpoint has rate limiting configured:
- 1 connection per IP
- 60 requests per minute
- Burst multiplier of 3

Plan your queries accordingly and implement exponential backoff for retries. 