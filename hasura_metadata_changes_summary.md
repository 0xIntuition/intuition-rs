# Hasura Metadata Changes Summary

## Overview
Updated all Hasura metadata files to remove foreign key constraint relationships and replace them with manual configuration relationships, consistent with the database schema changes that removed all foreign key constraints.

## Tables Updated

### Core Entity Tables
- **public_atom.yaml**: Updated 6 relationships
  - `creator` → manual config (creator_id → account.id)
  - `term` → manual config (term_id → term.id)
  - `value` → manual config (term_id → atom_value.id)
  - `accounts` → manual config (term_id → account.atom_id)
  - `as_object_predicate_objects` → manual config (term_id → predicate_object.object_id)
  - `as_object_triples` → manual config (term_id → triple.object_id)
  - `as_predicate_predicate_objects` → manual config (term_id → predicate_object.predicate_id)
  - `as_predicate_triples` → manual config (term_id → triple.predicate_id)
  - `as_subject_triples` → manual config (term_id → triple.subject_id)

- **public_account.yaml**: Updated 7 relationships
  - `atom` → manual config (atom_id → atom.term_id)
  - `atoms` → manual config (id → atom.creator_id)
  - `deposits_received` → manual config (id → deposit.receiver_id)
  - `deposits_sent` → manual config (id → deposit.sender_id)
  - `fee_transfers` → manual config (id → fee_transfer.sender_id)
  - `positions` → manual config (id → position.account_id)
  - `redemptions_received` → manual config (id → redemption.receiver_id)
  - `redemptions_sent` → manual config (id → redemption.sender_id)
  - `signals` → manual config (id → signal.account_id)
  - `triples` → manual config (id → triple.creator_id)

- **public_triple.yaml**: Updated 3 relationships
  - `object` → manual config (object_id → atom.term_id)
  - `predicate` → manual config (predicate_id → atom.term_id)
  - `subject` → manual config (subject_id → atom.term_id)

- **public_term.yaml**: Updated 8 relationships
  - `atomById` → manual config (id → atom.term_id)
  - `tripleById` → manual config (id → triple.term_id)
  - `deposits` → manual config (id → deposit.term_id)
  - `positions` → manual config (id → position.term_id)
  - `redemptions` → manual config (id → redemption.term_id)
  - `share_price_changes` → manual config (id → share_price_change.term_id)
  - `signals` → manual config (id → signal.term_id)
  - `vaults` → manual config (id → vault.term_id)

### Transaction Tables
- **public_deposit.yaml**: Updated 2 relationships
  - `receiver` → manual config (receiver_id → account.id)
  - `term` → manual config (term_id → term.id)

- **public_redemption.yaml**: Updated 2 relationships
  - `receiver` → manual config (receiver_id → account.id)
  - `term` → manual config (term_id → term.id)

- **public_fee_transfer.yaml**: Updated 1 relationship
  - `receiver` → manual config (receiver_id → account.id)

- **public_event.yaml**: Updated 3 relationships
  - `deposit` → manual config (deposit_id → deposit.id)
  - `fee_transfer` → manual config (fee_transfer_id → fee_transfer.id)
  - `redemption` → manual config (redemption_id → redemption.id)

### Vault System Tables
- **public_vault.yaml**: Updated 1 relationship
  - `term` → manual config (term_id → term.id)

- **public_position.yaml**: Updated 1 relationship
  - `term` → manual config (term_id → term.id)

- **public_signal.yaml**: Updated 3 relationships
  - `deposit` → manual config (deposit_id → deposit.id)
  - `redemption` → manual config (redemption_id → redemption.id)
  - `term` → manual config (term_id → term.id)

- **public_share_price_change.yaml**: Updated 1 relationship
  - `term` → manual config (term_id → term.id)

### Aggregate Tables
- **public_predicate_object.yaml**: Updated 2 relationships
  - `object` → manual config (object_id → atom.term_id)
  - `predicate` → manual config (predicate_id → atom.term_id)

- **public_subject_predicate.yaml**: Updated 2 relationships
  - `predicate` → manual config (predicate_id → atom.term_id)
  - `subject` → manual config (subject_id → atom.term_id)

### Value Object Tables
- **public_atom_value.yaml**: Updated 3 relationships
  - `account` → manual config (account_id → account.id)
  - `atom` → manual config (id → atom.term_id)
  - `caip10` → manual config (caip10_id → caip10.id)

## Key Changes Made

1. **Replaced `foreign_key_constraint_on`** with `manual_configuration`
2. **Added proper column mapping** for all relationships
3. **Set `insertion_order: null`** for all manual relationships
4. **Specified remote table and schema** for all relationships
5. **Maintained all existing relationship names** and functionality

## Benefits

- **Consistency**: Metadata now matches the database schema without foreign keys
- **Flexibility**: Manual relationships allow for more complex join logic
- **Performance**: Indexes provide the same query performance as foreign keys
- **Maintainability**: Relationships are explicitly defined in metadata

## Next Steps

1. **Reload Hasura metadata** to apply these changes
2. **Test GraphQL queries** to ensure relationships work correctly
3. **Verify performance** of joins and queries
4. **Update any custom resolvers** if needed

All relationships should continue to work exactly as before, but now using manual configuration instead of foreign key constraints.
