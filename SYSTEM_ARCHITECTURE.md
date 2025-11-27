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

**atom**: Stores atom records
- `term_id` (PK): Unique atom identifier
- `wallet_id`: Associated atom wallet address
- `creator_id`: Creator account address
- `data`: Decoded atom data (JSON string or text)
- `raw_data`: Raw hex-encoded atom data
- `type`: Atom type (Account, Image, Text, JSON, Unknown)
- `label`: Human-readable label
- `image`: Image URL or IPFS hash
- `emoji`: Emoji representation
- `resolving_status`: Status (Pending, Resolved, Failed)
- `block_number`, `created_at`, `transaction_hash`

**triple**: Stores triple (subject-predicate-object) records
- `term_id` (PK): Unique triple identifier
- `subject_id`: Subject atom term_id
- `predicate_id`: Predicate atom term_id
- `object_id`: Object atom term_id
- `block_number`, `created_at`, `transaction_hash`

**vault**: Stores vault state for atoms and triples
- `term_id` + `curve_id` (PK): Composite key
- `total_assets`: Total assets in vault
- `total_shares`: Total shares in vault
- `vault_type`: Atom or Triple
- `block_number`, `created_at`, `transaction_hash`

**deposit**: Records all deposits
- `id` (PK): Event ID
- `sender_id`: Depositor account
- `receiver_id`: Share recipient account
- `term_id`: Vault term ID
- `curve_id`: Bonding curve ID
- `assets_after_fees`: Assets after fees
- `shares`: Shares minted
- `total_shares`: Total shares after deposit
- `vault_type`: Atom or Triple
- `block_number`, `created_at`, `transaction_hash`, `log_index`

**redemption**: Records all redemptions
- `id` (PK): Event ID
- `sender_id`: Share redeemer account
- `receiver_id`: Asset recipient account
- `term_id`: Vault term ID
- `curve_id`: Bonding curve ID
- `shares`: Shares redeemed
- `assets`: Assets received
- `fees`: Fees deducted
- `total_shares`: Total shares after redemption
- `vault_type`: Atom or Triple
- `block_number`, `created_at`, `transaction_hash`, `log_index`

**position**: Tracks user positions in vaults
- `account_id` + `term_id` + `curve_id` (PK): Composite key
- `shares`: User's share balance
- `block_number`, `created_at`, `transaction_hash`

**account**: Stores account information
- `id` (PK): Account address
- `atom_id`: Associated atom term_id (if account atom)
- `label`: ENS name or short address
- `image`: Profile image URL
- `type`: Account type (User, AtomWallet, etc.)
- `created_at`, `updated_at`

**event**: Event log for all processed events
- `id` (PK): Event ID (transaction_hash + log_index)
- `event_type`: Type of event
- `atom_id`, `triple_id`, `deposit_id`, `redemption_id`: Optional foreign keys
- `block_number`, `created_at`, `transaction_hash`

**signal**: Activity signals for deposits/redemptions
- `id` (PK): Signal ID
- `term_id`: Vault term ID
- `account_id`: Account involved
- `signal_type`: Deposit or Redemption
- `block_number`, `created_at`, `transaction_hash`

**share_price_change**: Historical share price changes
- `term_id` + `curve_id` + `block_number` (PK): Composite key
- `share_price`: Share price at this point
- `total_assets`: Total assets at this point
- `total_shares`: Total shares at this point
- `vault_type`: Atom or Triple
- `block_timestamp`, `transaction_hash`, `log_index`

**stats**: System-wide statistics
- `id` (PK): Always 1
- `total_accounts`, `total_atoms`, `total_triples`, `total_positions`, `total_signals`
- `total_fees`: Accumulated protocol fees
- `contract_balance`: Current contract balance
- `last_processed_block_number`, `last_processed_block_timestamp`

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

