# System Architecture: Event Processing Pipeline

## Overview

The system processes blockchain events from the MultiVault v2.0 smart contract through a multi-stage consumer pipeline. Events flow through three distinct consumer modes: **Decoded**, **Resolver**, and **IpfsUpload**, each handling different aspects of event processing and data enrichment.

## Consumer Pipeline Flow

```
Raw logs
    ↓
[Decoded Consumer] → Decode logs → Processes events → Creates DB records → Sends to Resolver queue (if needed)
    ↓
[Resolver Consumer] → Resolves IPFS/ENS data → Updates metadata → Sends to IpfsUpload queue (if needed)
    ↓
[IpfsUpload Consumer] → Downloads & classifies images → Stores in IPFS
```

## Consumer Modes

### 1. Decoded Consumer

**Purpose**: Processes Raw logs and creates/updates database records.

**Responsibilities**:
- Receives raw log from the indexer
- Decode it into an event (AtomCreated, TripleCreated, ...)
- Processes each event type with specific handlers
- Creates/updates database records (atoms, triples, deposits, redemptions, etc.)
- Updates vault states and positions
- Sends atoms/accounts to Resolver queue for metadata resolution
- Updates system stats (block number, contract balance)

**Input**: Raw logs
**Output**: Database records + messages to Resolver queue

**Event Handlers**:
- `AtomCreatedEventHandler`: Creates atoms and vaults
- `TripleCreatedEventHandler`: Creates triples and relationships
- `DepositedEventHandler`: Records deposits and updates positions
- `RedeemedEventHandler`: Records redemptions and updates positions
- `SharePriceChangedEventHandler`: Updates vault share prices
- `InitializedEventHandler`: Records contract initialization

### 2. Resolver Consumer

**Purpose**: Resolves external data (IPFS, ENS) and enriches atom/account metadata.

**Responsibilities**:
- Receives atom IDs or account objects from Decoded consumer
- Resolves IPFS URIs to fetch atom data
- Parses JSON/text atom data to determine atom type
- Resolves ENS names for account addresses
- Updates atom/account metadata (label, image, type, etc.)
- Handles binary data (images) by sending to IpfsUpload queue
- Marks atoms as resolved or failed

**Input**: Atom IDs or Account objects
**Output**: Updated metadata in database + messages to IpfsUpload queue (for images)

**Key Functions**:
- `process_atom()`: Resolves atom data from IPFS or direct JSON
- `process_account()`: Resolves ENS names for accounts
- `resolve_and_parse_atom_data()`: Fetches and parses atom data
- `handle_atom_image()`: Sends image URLs to IpfsUpload queue

### 3. IpfsUpload Consumer

**Purpose**: Downloads, classifies, and stores images in IPFS.

**Responsibilities**:
- Receives image URLs from Resolver consumer
- Downloads images from URLs
- Classifies images (NSFW detection, content validation)
- Uploads validated images to IPFS
- Handles failures gracefully (logs warnings)

**Input**: Image URLs
**Output**: Images stored in IPFS (via image-guard service)

## Events Processed

The system processes the following events from the MultiVault v2.0 contract:

### 1. AtomCreated

**Event Signature**: `AtomCreated(address indexed creator, bytes32 indexed termId, bytes atomData, address atomWallet)`

**Data Extracted**:
- `creator`: Address of the atom creator
- `termId`: Unique identifier for the atom (bytes32)
- `atomData`: Raw atom data (bytes, can be hex-encoded JSON or IPFS URI)
- `atomWallet`: Address of the associated atom wallet

**Processing**:
1. Creates `Atom` record with term_id, creator_id, wallet_id, raw_data
2. Decodes atom data (hex to string)
3. Creates `Account` record for the atom wallet
4. Extracts supported metadata (if JSON)
5. Sends atom to Resolver queue for full resolution
6. Creates `Event` record with type `AtomCreated`

**Database Tables Updated**:
- `atom`: New atom record
- `account`: Atom wallet account
- `event`: AtomCreated event record

**Note**: Vaults are not created here. They are created later when `SharePriceChanged` events occur.

### 2. TripleCreated

**Event Signature**: `TripleCreated(address indexed creator, bytes32 indexed termId, bytes32 subjectId, bytes32 predicateId, bytes32 objectId)`

**Data Extracted**:
- `creator`: Address of the triple creator
- `termId`: Unique identifier for the triple (bytes32)
- `subjectId`: Subject atom ID (bytes32)
- `predicateId`: Predicate atom ID (bytes32)
- `objectId`: Object atom ID (bytes32)

**Processing**:
1. Creates `Triple` record with term_id and component atom IDs
2. Checks and updates account/predicate/object relationships
3. Creates `Event` record with type `TripleCreated`

**Database Tables Updated**:
- `triple`: New triple record
- `event`: TripleCreated event record
- `account`: May update account relationships

**Note**: Vaults are not created here. They are created later when `SharePriceChanged` events occur.

### 3. Deposited

**Event Signature**: `Deposited(address indexed sender, address indexed receiver, bytes32 indexed termId, uint256 curveId, uint256 assets, uint256 assetsAfterFees, uint256 shares, uint256 totalShares, uint8 vaultType)`

**Data Extracted**:
- `sender`: Address depositing assets
- `receiver`: Address receiving shares
- `termId`: Vault term ID (bytes32)
- `curveId`: Bonding curve ID (uint256)
- `assets`: Amount of assets deposited
- `assetsAfterFees`: Assets after fees deducted
- `shares`: Shares minted
- `totalShares`: Total shares in vault after deposit
- `vaultType`: Type of vault (Atom or Triple)

**Processing**:
1. Creates/updates `Account` records for sender and receiver
2. Creates `Deposit` record with all deposit details
3. Updates `Vault` totals (totalAssets, totalShares) - vault must already exist
4. Creates/updates `Position` records for the receiver
5. Handles atom re-resolution if needed
6. Creates `Signal` record for the deposit
7. Creates `Event` record with type `Deposited`

**Database Tables Updated**:
- `deposit`: New deposit record
- `vault`: Updated totals (vault must exist - created by SharePriceChanged)
- `position`: Created/updated position for receiver
- `account`: Sender and receiver accounts
- `signal`: Deposit signal
- `event`: Deposited event record

### 4. Redeemed

**Event Signature**: `Redeemed(address indexed sender, address indexed receiver, bytes32 indexed termId, uint256 curveId, uint256 shares, uint256 totalShares, uint256 assets, uint256 fees, uint8 vaultType)`

**Data Extracted**:
- `sender`: Address redeeming shares
- `receiver`: Address receiving assets
- `termId`: Vault term ID (bytes32)
- `curveId`: Bonding curve ID (uint256)
- `shares`: Shares redeemed
- `totalShares`: Total shares in vault after redemption
- `assets`: Assets received
- `fees`: Fees deducted
- `vaultType`: Type of vault (Atom or Triple)

**Processing**:
1. Creates/updates `Account` records for sender and receiver
2. Creates `Redemption` record with all redemption details
3. Updates `Vault` totals (totalAssets, totalShares)
4. Updates `Position` records (decreases shares)
5. Creates `Signal` record for the redemption
6. Creates `Event` record with type `Redeemed`

**Database Tables Updated**:
- `redemption`: New redemption record
- `vault`: Updated totals
- `position`: Updated position (shares decreased)
- `account`: Sender and receiver accounts
- `signal`: Redemption signal
- `event`: Redeemed event record

### 5. SharePriceChanged

**Event Signature**: `SharePriceChanged(bytes32 indexed termId, uint256 indexed curveId, uint256 sharePrice, uint256 totalAssets, uint256 totalShares, uint8 vaultType)`

**Data Extracted**:
- `termId`: Vault term ID (bytes32)
- `curveId`: Bonding curve ID (uint256)
- `sharePrice`: New share price (uint256)
- `totalAssets`: Total assets in vault (uint256)
- `totalShares`: Total shares in vault (uint256)
- `vaultType`: Type of vault (Atom or Triple)

**Processing**:
1. Checks if `Vault` exists for the term_id and curve_id
2. If vault exists: Updates `Vault` record with new totals and share price
3. If vault doesn't exist: Creates new `Vault` record (this is where vaults are initially created)
4. Creates `SharePriceChange` record for historical tracking
5. Updates share price aggregates

**Database Tables Updated**:
- `vault`: Created (if new) or updated totals and share price
- `share_price_change`: Historical price change record

### 6. Initialized

**Event Signature**: `Initialized(uint64 version)`

**Data Extracted**:
- `version`: Contract initialization version (uint64)

**Processing**:
1. Creates `Event` record with type `Initialized`

**Database Tables Updated**:
- `event`: Initialized event record

## Database Schema Overview

### Core Tables

#### atom

Stores atom records.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `term_id` | TEXT | Unique atom identifier | PRIMARY KEY |
| `wallet_id` | TEXT | Associated atom wallet address | NOT NULL |
| `creator_id` | TEXT | Creator account address | NOT NULL |
| `data` | TEXT | Decoded atom data (JSON string or text) | |
| `raw_data` | TEXT | Raw hex-encoded atom data | |
| `type` | TEXT | Atom type | CHECK: Account, Image, Text, JSON, Unknown |
| `label` | TEXT | Human-readable label | |
| `image` | TEXT | Image URL or IPFS hash | |
| `emoji` | TEXT | Emoji representation | |
| `resolving_status` | TEXT | Status | CHECK: Pending, Resolved, Failed |
| `block_number` | BIGINT | Block number where atom was created | NOT NULL |
| `created_at` | TIMESTAMPTZ | Creation timestamp | DEFAULT NOW() |
| `transaction_hash` | TEXT | Transaction hash | NOT NULL |

#### triple

Stores triple (subject-predicate-object) records.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `term_id` | TEXT | Unique triple identifier | PRIMARY KEY |
| `subject_id` | TEXT | Subject atom term_id | NOT NULL, FK → atom(term_id) |
| `predicate_id` | TEXT | Predicate atom term_id | NOT NULL, FK → atom(term_id) |
| `object_id` | TEXT | Object atom term_id | NOT NULL, FK → atom(term_id) |
| `block_number` | BIGINT | Block number where triple was created | NOT NULL |
| `created_at` | TIMESTAMPTZ | Creation timestamp | DEFAULT NOW() |
| `transaction_hash` | TEXT | Transaction hash | NOT NULL |

#### vault

Stores vault state for atoms and triples.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `term_id` | TEXT | Vault term ID | PRIMARY KEY (composite) |
| `curve_id` | NUMERIC(78,0) | Bonding curve ID | PRIMARY KEY (composite) |
| `total_assets` | NUMERIC(78,0) | Total assets in vault | NOT NULL, DEFAULT 0 |
| `total_shares` | NUMERIC(78,0) | Total shares in vault | NOT NULL, DEFAULT 0 |
| `vault_type` | TEXT | Type of vault | NOT NULL, CHECK: Atom, Triple |
| `block_number` | BIGINT | Block number | NOT NULL |
| `created_at` | TIMESTAMPTZ | Creation timestamp | DEFAULT NOW() |
| `transaction_hash` | TEXT | Transaction hash | NOT NULL |

#### deposit

Records all deposits.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `id` | TEXT | Event ID | PRIMARY KEY |
| `sender_id` | TEXT | Depositor account | NOT NULL, FK → account(id) |
| `receiver_id` | TEXT | Share recipient account | NOT NULL, FK → account(id) |
| `term_id` | TEXT | Vault term ID | NOT NULL, FK → vault(term_id) |
| `curve_id` | NUMERIC(78,0) | Bonding curve ID | NOT NULL, FK → vault(curve_id) |
| `assets_after_fees` | NUMERIC(78,0) | Assets after fees | NOT NULL |
| `shares` | NUMERIC(78,0) | Shares minted | NOT NULL |
| `total_shares` | NUMERIC(78,0) | Total shares after deposit | NOT NULL |
| `vault_type` | TEXT | Type of vault | NOT NULL, CHECK: Atom, Triple |
| `block_number` | BIGINT | Block number | NOT NULL |
| `created_at` | TIMESTAMPTZ | Creation timestamp | DEFAULT NOW() |
| `transaction_hash` | TEXT | Transaction hash | NOT NULL |
| `log_index` | INTEGER | Log index in transaction | NOT NULL |

#### redemption

Records all redemptions.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `id` | TEXT | Event ID | PRIMARY KEY |
| `sender_id` | TEXT | Share redeemer account | NOT NULL, FK → account(id) |
| `receiver_id` | TEXT | Asset recipient account | NOT NULL, FK → account(id) |
| `term_id` | TEXT | Vault term ID | NOT NULL, FK → vault(term_id) |
| `curve_id` | NUMERIC(78,0) | Bonding curve ID | NOT NULL, FK → vault(curve_id) |
| `shares` | NUMERIC(78,0) | Shares redeemed | NOT NULL |
| `assets` | NUMERIC(78,0) | Assets received | NOT NULL |
| `fees` | NUMERIC(78,0) | Fees deducted | NOT NULL |
| `total_shares` | NUMERIC(78,0) | Total shares after redemption | NOT NULL |
| `vault_type` | TEXT | Type of vault | NOT NULL, CHECK: Atom, Triple |
| `block_number` | BIGINT | Block number | NOT NULL |
| `created_at` | TIMESTAMPTZ | Creation timestamp | DEFAULT NOW() |
| `transaction_hash` | TEXT | Transaction hash | NOT NULL |
| `log_index` | INTEGER | Log index in transaction | NOT NULL |

#### position

Tracks user positions in vaults.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `account_id` | TEXT | Account address | PRIMARY KEY (composite), FK → account(id) |
| `term_id` | TEXT | Vault term ID | PRIMARY KEY (composite), FK → vault(term_id) |
| `curve_id` | NUMERIC(78,0) | Bonding curve ID | PRIMARY KEY (composite), FK → vault(curve_id) |
| `shares` | NUMERIC(78,0) | User's share balance | NOT NULL, DEFAULT 0 |
| `block_number` | BIGINT | Block number | NOT NULL |
| `created_at` | TIMESTAMPTZ | Creation timestamp | DEFAULT NOW() |
| `transaction_hash` | TEXT | Transaction hash | NOT NULL |

#### account

Stores account information.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `id` | TEXT | Account address | PRIMARY KEY |
| `atom_id` | TEXT | Associated atom term_id (if account atom) | FK → atom(term_id) |
| `label` | TEXT | ENS name or short address | |
| `image` | TEXT | Profile image URL | |
| `type` | TEXT | Account type (User, AtomWallet, etc.) | |
| `created_at` | TIMESTAMPTZ | Creation timestamp | DEFAULT NOW() |
| `updated_at` | TIMESTAMPTZ | Last update timestamp | DEFAULT NOW() |

#### event

Event log for all processed events.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `id` | TEXT | Event ID (transaction_hash + log_index) | PRIMARY KEY |
| `event_type` | TEXT | Type of event | NOT NULL |
| `atom_id` | TEXT | Reference to atom | FK → atom(term_id) |
| `triple_id` | TEXT | Reference to triple | FK → triple(term_id) |
| `deposit_id` | TEXT | Reference to deposit | FK → deposit(id) |
| `redemption_id` | TEXT | Reference to redemption | FK → redemption(id) |
| `block_number` | BIGINT | Block number | NOT NULL |
| `created_at` | TIMESTAMPTZ | Creation timestamp | DEFAULT NOW() |
| `transaction_hash` | TEXT | Transaction hash | NOT NULL |

#### signal

Activity signals for deposits/redemptions.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `id` | BIGSERIAL | Signal ID | PRIMARY KEY |
| `term_id` | TEXT | Vault term ID | NOT NULL |
| `account_id` | TEXT | Account involved | NOT NULL, FK → account(id) |
| `signal_type` | TEXT | Type of signal | NOT NULL, CHECK: Deposit, Redemption |
| `block_number` | BIGINT | Block number | NOT NULL |
| `created_at` | TIMESTAMPTZ | Creation timestamp | DEFAULT NOW() |
| `transaction_hash` | TEXT | Transaction hash | NOT NULL |

#### share_price_change

Historical share price changes.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `term_id` | TEXT | Vault term ID | PRIMARY KEY (composite), FK → vault(term_id) |
| `curve_id` | NUMERIC(78,0) | Bonding curve ID | PRIMARY KEY (composite), FK → vault(curve_id) |
| `block_number` | BIGINT | Block number | PRIMARY KEY (composite) |
| `share_price` | NUMERIC(78,0) | Share price at this point | NOT NULL |
| `total_assets` | NUMERIC(78,0) | Total assets at this point | NOT NULL |
| `total_shares` | NUMERIC(78,0) | Total shares at this point | NOT NULL |
| `vault_type` | TEXT | Type of vault | NOT NULL, CHECK: Atom, Triple |
| `block_timestamp` | TIMESTAMPTZ | Block timestamp | NOT NULL |
| `transaction_hash` | TEXT | Transaction hash | NOT NULL |
| `log_index` | INTEGER | Log index in transaction | NOT NULL |

#### stats

System-wide statistics.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `id` | INTEGER | Always 1 | PRIMARY KEY, CHECK: id = 1 |
| `total_accounts` | BIGINT | Total number of accounts | NOT NULL, DEFAULT 0 |
| `total_atoms` | BIGINT | Total number of atoms | NOT NULL, DEFAULT 0 |
| `total_triples` | BIGINT | Total number of triples | NOT NULL, DEFAULT 0 |
| `total_positions` | BIGINT | Total number of positions | NOT NULL, DEFAULT 0 |
| `total_signals` | BIGINT | Total number of signals | NOT NULL, DEFAULT 0 |
| `total_fees` | NUMERIC(78,0) | Accumulated protocol fees | NOT NULL, DEFAULT 0 |
| `contract_balance` | NUMERIC(78,0) | Current contract balance | NOT NULL, DEFAULT 0 |
| `last_processed_block_number` | BIGINT | Last processed block number | |
| `last_processed_block_timestamp` | TIMESTAMPTZ | Last processed block timestamp | |

### Aggregate Tables

The system maintains several aggregate tables that pre-compute totals across related entities. These aggregates are updated automatically via database triggers when underlying data changes.

#### term: Unified Term View

**Purpose**: Provides a unified view of all terms (atoms and triples) in the system.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `id` | TEXT | Term identifier (same as atom.term_id or triple.term_id) | PRIMARY KEY |
| `type` | TEXT | Term type enum | NOT NULL, CHECK: Atom, Triple, CounterTriple |
| `atom_id` | TEXT | Reference to atom table (if type is `Atom`) | FK → atom(term_id) |
| `triple_id` | TEXT | Reference to triple table (if type is `Triple` or `CounterTriple`) | FK → triple(term_id) |
| `total_assets` | NUMERIC(78,0) | Aggregated total assets across all vaults for this term | DEFAULT 0 |
| `total_market_cap` | NUMERIC(78,0) | Aggregated market cap across all vaults for this term | DEFAULT 0 |
| `created_at` | TIMESTAMPTZ | Creation timestamp | DEFAULT NOW() |
| `updated_at` | TIMESTAMPTZ | Last update timestamp | DEFAULT NOW() |

**Key Points**:
- Every atom and triple has a corresponding `term` record
- Acts as a unified interface for querying both atoms and triples
- `total_assets` and `total_market_cap` are aggregates computed from vaults
- Used by `triple_vault` and `triple_term` for relationships

**Example**:
- Atom with `term_id = 0x123...` → `term` record with `id = 0x123...`, `type = 'Atom'`, `atom_id = 0x123...`
- Triple with `term_id = 0x456...` → `term` record with `id = 0x456...`, `type = 'Triple'`, `triple_id = 0x456...`

#### triple_vault: Per-Curve Triple Aggregates

**Purpose**: Aggregates vault data for a specific bonding curve across both sides of a triple relationship.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `term_id` | TEXT | The triple's main term ID | PRIMARY KEY (composite), FK → term(id) |
| `counter_term_id` | TEXT | The triple's counter term ID | PRIMARY KEY (composite), FK → term(id) |
| `curve_id` | NUMERIC(78,0) | Bonding curve ID | PRIMARY KEY (composite) |
| `total_shares` | NUMERIC(78,0) | Sum of `total_shares` from vaults for both `term_id` and `counter_term_id` for this `curve_id` | DEFAULT 0 |
| `total_assets` | NUMERIC(78,0) | Sum of `total_assets` from vaults for both `term_id` and `counter_term_id` for this `curve_id` | DEFAULT 0 |
| `market_cap` | NUMERIC(78,0) | Sum of `market_cap` from vaults for both `term_id` and `counter_term_id` for this `curve_id` | DEFAULT 0 |
| `position_count` | BIGINT | Sum of `position_count` from vaults for both `term_id` and `counter_term_id` for this `curve_id` | DEFAULT 0 |
| `block_number` | BIGINT | Block number | |
| `log_index` | INTEGER | Log index | |
| `updated_at` | TIMESTAMPTZ | Last update timestamp | DEFAULT NOW() |

**How It Works**:
- For each triple, there are two vaults: one for `term_id` and one for `counter_term_id`
- `triple_vault` aggregates data from **both** vaults for a specific `curve_id`
- Updated automatically via triggers when `vault` records change
- One record per `(term_id, counter_term_id, curve_id)` combination

**Example**:
```
Triple: term_id = 0xABC, counter_term_id = 0xDEF
Vault 1: term_id = 0xABC, curve_id = 1, total_assets = 1000
Vault 2: term_id = 0xDEF, curve_id = 1, total_assets = 2000

triple_vault record:
  term_id = 0xABC
  counter_term_id = 0xDEF
  curve_id = 1
  total_assets = 3000 (1000 + 2000)
```

**Maintenance**:
- Trigger `update_triple_vault_from_vault()` fires on `vault` INSERT/UPDATE/DELETE
- Recalculates aggregates by summing vault data where `vault.term_id IN (triple_vault.term_id, triple_vault.counter_term_id)`

#### triple_term: Cross-Curve Triple Aggregates

**Purpose**: Aggregates vault data across **all curves** for a triple relationship.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `term_id` | TEXT | The triple's main term ID | PRIMARY KEY, FK → term(id) |
| `counter_term_id` | TEXT | The triple's counter term ID | NOT NULL, FK → term(id) |
| `total_assets` | NUMERIC(78,0) | Sum of `total_assets` from all vaults for both `term_id` and `counter_term_id` across **all curves** | DEFAULT 0 |
| `total_market_cap` | NUMERIC(78,0) | Sum of `market_cap` from all vaults for both `term_id` and `counter_term_id` across **all curves** | DEFAULT 0 |
| `total_position_count` | BIGINT | Sum of `position_count` from all vaults for both `term_id` and `counter_term_id` across **all curves** | DEFAULT 0 |
| `updated_at` | TIMESTAMPTZ | Last update timestamp | DEFAULT NOW() |

**How It Works**:
- Aggregates data from **all curves** (not just one curve like `triple_vault`)
- One record per `(term_id, counter_term_id)` pair
- Updated automatically via triggers when `vault` records change
- Provides a single view of total activity across all bonding curves for a triple

**Example**:
```
Triple: term_id = 0xABC, counter_term_id = 0xDEF
Vault 1: term_id = 0xABC, curve_id = 1, total_assets = 1000
Vault 2: term_id = 0xDEF, curve_id = 1, total_assets = 2000
Vault 3: term_id = 0xABC, curve_id = 2, total_assets = 500
Vault 4: term_id = 0xDEF, curve_id = 2, total_assets = 1500

triple_term record:
  term_id = 0xABC
  counter_term_id = 0xDEF
  total_assets = 5000 (1000 + 2000 + 500 + 1500)
```

**Maintenance**:
- Trigger `update_triple_vault_from_vault()` also updates `triple_term` when vaults change
- Recalculates by summing vault data where `vault.term_id IN (triple_term.term_id, triple_term.counter_term_id)` (across all curves)

### Aggregate Hierarchy

The aggregate tables form a hierarchy:

```
vault (per term_id, per curve_id)
    ↓
triple_vault (per term_id + counter_term_id, per curve_id)
    ↓
triple_term (per term_id + counter_term_id, all curves)
```

**Query Patterns**:
- **Per-curve triple data**: Query `triple_vault` with specific `curve_id`
- **All-curves triple data**: Query `triple_term`
- **Individual vault data**: Query `vault` directly
- **Unified term view**: Query `term` to get atom or triple details

**Performance Benefits**:
- Pre-computed aggregates avoid expensive JOINs and SUMs at query time
- Fast lookups for dashboard queries and analytics
- Maintained automatically via triggers (no application code needed)

#### predicate_object: Predicate-Object Pair Aggregates

**Purpose**: Aggregates data for all triples that share the same predicate-object pair.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `predicate_id` | TEXT | Atom term_id used as predicate | PRIMARY KEY (composite), FK → atom(term_id) |
| `object_id` | TEXT | Atom term_id used as object | PRIMARY KEY (composite), FK → atom(term_id) |
| `triple_count` | BIGINT | Count of triples with this predicate-object pair | DEFAULT 0 |
| `total_position_count` | BIGINT | Sum of `total_position_count` from all `triple_term` records for triples matching this predicate-object pair | DEFAULT 0 |
| `total_market_cap` | NUMERIC(78,0) | Sum of `total_market_cap` from all `triple_term` records for triples matching this predicate-object pair | DEFAULT 0 |

**How It Works**:
- Groups triples by their `(predicate_id, object_id)` combination
- Aggregates position counts and market caps from `triple_term` table
- Useful for queries like "all triples where predicate=X and object=Y"

**Example**:
```
Triple 1: predicate_id = 0xPRED, object_id = 0xOBJ, triple_term.total_position_count = 100
Triple 2: predicate_id = 0xPRED, object_id = 0xOBJ, triple_term.total_position_count = 50
Triple 3: predicate_id = 0xPRED, object_id = 0xOBJ, triple_term.total_position_count = 75

predicate_object record:
  predicate_id = 0xPRED
  object_id = 0xOBJ
  triple_count = 3
  total_position_count = 225 (100 + 50 + 75)
```

**Maintenance**:
- Updated via triggers when `triple_term` records change
- Recalculates by summing `triple_term` data where `triple.predicate_id` and `triple.object_id` match

#### subject_predicate: Subject-Predicate Pair Aggregates

**Purpose**: Aggregates data for all triples that share the same subject-predicate pair.

| Column | Type | Description | Constraints |
|--------|------|-------------|-------------|
| `subject_id` | TEXT | Atom term_id used as subject | PRIMARY KEY (composite), FK → atom(term_id) |
| `predicate_id` | TEXT | Atom term_id used as predicate | PRIMARY KEY (composite), FK → atom(term_id) |
| `triple_count` | BIGINT | Count of triples with this subject-predicate pair | DEFAULT 0 |
| `total_position_count` | BIGINT | Sum of `total_position_count` from all `triple_term` records for triples matching this subject-predicate pair | DEFAULT 0 |
| `total_market_cap` | NUMERIC(78,0) | Sum of `total_market_cap` from all `triple_term` records for triples matching this subject-predicate pair | DEFAULT 0 |

**How It Works**:
- Groups triples by their `(subject_id, predicate_id)` combination
- Aggregates position counts and market caps from `triple_term` table
- Useful for queries like "all triples where subject=X and predicate=Y"

**Example**:
```
Triple 1: subject_id = 0xSUBJ, predicate_id = 0xPRED, triple_term.total_position_count = 200
Triple 2: subject_id = 0xSUBJ, predicate_id = 0xPRED, triple_term.total_position_count = 150

subject_predicate record:
  subject_id = 0xSUBJ
  predicate_id = 0xPRED
  triple_count = 2
  total_position_count = 350 (200 + 150)
```

**Maintenance**:
- Updated via triggers when `triple_term` records change
- Recalculates by summing `triple_term` data where `triple.subject_id` and `triple.predicate_id` match

### Complete Aggregate Hierarchy

The aggregate tables form a complete hierarchy from individual vaults to semantic groupings:

```
vault (per term_id, per curve_id)
    ↓
triple_vault (per term_id + counter_term_id, per curve_id)
    ↓
triple_term (per term_id + counter_term_id, all curves)
    ↓
predicate_object (per predicate_id + object_id, all matching triples)
subject_predicate (per subject_id + predicate_id, all matching triples)
```

**Use Cases**:
- **triple_vault**: "Show me vault stats for this triple on curve 1"
- **triple_term**: "Show me total activity for this triple across all curves"
- **predicate_object**: "Show me all triples with predicate 'follows' and object 'person X'"
- **subject_predicate**: "Show me all triples where subject 'person X' has predicate 'follows'"

## Data Flow Examples

### Example 1: Atom Creation Flow

1. **Decoded Consumer**: 
   - Receives decoded `AtomCreated` event
   - Creates `Atom` record with raw data
   - Creates `Account` for atom wallet
   - Decodes hex data to string
   - Extracts basic metadata if JSON
   - Sends atom ID to Resolver queue
   - Creates `Event` record
   - **Note**: Vault is not created yet
2. **Resolver Consumer**:
   - Fetches atom data (IPFS or direct)
   - Parses JSON/text to determine type
   - Updates atom metadata (type, label, image, emoji)
   - If image found, sends image URL to IpfsUpload queue
   - Marks atom as resolved
3. **IpfsUpload Consumer**:
   - Downloads image
   - Classifies content
   - Uploads to IPFS
   - Updates atom with IPFS hash (if needed)
4. **Later, when SharePriceChanged event occurs**:
   - **Decoded Consumer** receives `SharePriceChanged` event
   - Creates `Vault` record (if it doesn't exist) or updates existing vault
   - Updates vault totals and share price

### Example 2: Deposit Flow

1. **Decoded Consumer**:
   - Receives decoded `Deposited` event
   - Creates/updates `Account` records (sender, receiver)
   - Creates `Deposit` record
   - Updates `Vault` totals
   - Creates/updates `Position` for receiver
   - Creates `Signal` record
   - Creates `Event` record
   - Updates `Stats` (block number, contract balance)

### Example 3: Triple Creation Flow

1. **Decoded Consumer**:
   - Receives decoded `TripleCreated` event
   - Creates `Triple` record with subject/predicate/object IDs
   - Updates account relationships if any component is an account atom
   - Creates `Event` record
   - **Note**: Vault is not created yet
2. **Later, when SharePriceChanged event occurs**:
   - **Decoded Consumer** receives `SharePriceChanged` event
   - Creates `Vault` record (if it doesn't exist) or updates existing vault
   - Updates vault totals and share price

### Example 4: Redemption Flow

1. **Decoded Consumer**:
   - Receives decoded `Redeemed` event
   - Creates/updates `Account` records (sender, receiver)
   - Creates `Redemption` record with all redemption details (shares, assets, fees)
   - Updates `Vault` totals (decreases `total_assets`, decreases `total_shares`)
   - Updates `Position` record for sender (decreases `shares` balance)
   - Creates `Signal` record for the redemption
   - Creates `Event` record with type `Redeemed`
   - Updates aggregate tables (`triple_vault`, `triple_term`) via triggers if applicable
   - Updates `Stats` (block number, contract balance)

**Key Differences from Deposit**:
- Decreases vault totals instead of increasing
- Decreases position shares instead of increasing
- Position may reach zero shares (position remains but is inactive)
- Fees are deducted from assets received

## Queue Communication

The consumers communicate via message queues (Redis Streams or Redis Hybrid):

- **Decoded → Resolver**: Atom IDs or Account objects
- **Resolver → IpfsUpload**: Image URLs

Each consumer can operate independently and process messages at its own pace, providing resilience and scalability.

## Error Handling

- **Database Errors**: Propagated as `ConsumerError::ModelError`
- **IPFS Resolution Failures**: Atom marked as failed, status updated
- **Image Upload Failures**: Logged as warnings, processing continues
- **Duplicate Events**: Checked by event ID, skipped if already processed

## Idempotency

All event processing is idempotent:
- Events are identified by unique IDs (transaction_hash + log_index)
- Database upserts prevent duplicate records
- Consumers check for existing records before processing
- This ensures safe reprocessing and recovery from failures

