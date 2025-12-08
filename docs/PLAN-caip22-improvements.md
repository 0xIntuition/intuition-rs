# Plan: CAIP-22 Improvements - Network Support & Fallback Label Fix

## Overview

This plan addresses two issues:
1. Add support for Polygon Amoy (chain ID 80002) and Ethereum Sepolia (chain ID 11155111) networks
2. Fix the fallback label for CAIP-22 atoms to use the token ID instead of "NFT"

## Issues Identified

### Issue 1: Missing Network Support
The CAIP-22 resolver currently supports these chains:
- Ethereum Mainnet (1)
- Ethereum Sepolia (11155111) - **partially supported** (code exists but RPC URL not passed to docker)
- Base Mainnet (8453)
- Base Sepolia (84532)
- Linea Mainnet (59144)
- Linea Sepolia (59141)
- Trust Mainnet (1155)
- Trust Testnet (13579)
- Local (31337)

**Missing:**
- Polygon Amoy (80002) - new testnet replacing Mumbai

### Issue 2: Fallback Label Uses "NFT" Instead of Token ID
When CAIP-22 resolution fails (e.g., `caip22:eip155:11155111/erc721:0x8004a6090Cd10A7288092483047B097295Fb8847/3265`), the label defaults to "NFT" instead of showing the token ID "3265".

Current code in `metadata.rs:77`:
```rust
pub fn caip22(name: Option<String>, image: Option<String>) -> Self {
    Self {
        label: name.unwrap_or_else(|| "NFT".to_string()),
        ...
    }
}
```

## Implementation Steps

### Step 1: Add Polygon Amoy RPC URL to Environment Config

**File:** `apps/consumer/src/config.rs`

Add new field to `Env` struct:
```rust
pub polygon_amoy_rpc_url: Option<String>,
```

### Step 2: Add Polygon Amoy to Chain RPC Mapping

**File:** `apps/consumer/src/mode/resolver/caip22_resolver.rs`

Add to `get_rpc_url_for_chain()` match statement:
```rust
80002 => env
    .polygon_amoy_rpc_url
    .clone()
    .ok_or(ConsumerError::ChainRpcNotConfigured(chain_id)),
```

### Step 3: Update Docker Compose to Pass Polygon Amoy RPC URL

**File:** `docker/docker-compose-apps.yml`

Add to `resolver_consumer` environment:
```yaml
POLYGON_AMOY_RPC_URL: $POLYGON_AMOY_RPC_URL
```

Also add to `prod-rpc-proxy` environment.

### Step 4: Update start.sh to Export Polygon Amoy RPC URL

**File:** `scripts/start.sh`

Add to the export list (around line 14):
```bash
export POLYGON_AMOY_RPC_URL
```

### Step 5: Update .env.sample with Polygon Amoy RPC URL

**File:** `.env.sample`

Add:
```
POLYGON_AMOY_RPC_URL=https://polygon-amoy.g.alchemy.com/v2/YOUR_API_KEY
```

### Step 6: Fix Fallback Label to Use Token ID

**File:** `apps/consumer/src/mode/metadata.rs`

Change the `caip22()` function signature to accept token_id:
```rust
/// Creates a new atom metadata for a CAIP-22 NFT
/// If name is None, uses the token_id as fallback label
pub fn caip22(name: Option<String>, image: Option<String>, token_id: Option<String>) -> Self {
    Self {
        label: name.unwrap_or_else(|| token_id.unwrap_or_else(|| "NFT".to_string())),
        emoji: "🖼️".to_string(),
        atom_type: "Caip22".to_string(),
        image,
    }
}
```

### Step 7: Update Callers of `AtomMetadata::caip22()`

**File:** `apps/consumer/src/mode/metadata.rs` (line 664)

Update the call in `get_supported_atom_metadata()`:
```rust
} else if is_valid_caip22(&atom.data.clone().ok_or(ConsumerError::AtomDataNotFound)?)? {
    let parsed = parse_caip22(&atom.data.clone().unwrap())?;
    Ok(AtomMetadata::caip22(None, None, Some(parsed.token_id)))
```

**File:** `apps/consumer/src/mode/resolver/caip22_resolver.rs` (line 170)

Update the call in `resolve_caip22()`:
```rust
let parsed = parse_caip22_data(atom)?;
// ... existing code ...
Ok(AtomMetadata::caip22(name, image, Some(parsed.token_id.clone())))
```

### Step 8: Add Ethereum Sepolia RPC URL to Docker Environment

**File:** `docker/docker-compose-apps.yml`

The `ETHEREUM_SEPOLIA_RPC_URL` is already in the docker-compose but needs to be exported in `start.sh`.

**File:** `scripts/start.sh`

The export already exists at line 14, but we need to ensure it's NOT overridden for local mode (similar to BASE_SEPOLIA_RPC_URL fix already done).

### Step 9: Add Integration Tests for New Networks

**File:** `integration-tests/src/create-agent.test.ts` (or new file)

Add test cases for:
1. Ethereum Sepolia CAIP-22 atom (`caip22:eip155:11155111/erc721:0x8004a6090Cd10A7288092483047B097295Fb8847/3265`)
2. Verify fallback label shows token ID when resolution fails

### Step 10: Add Rust Unit Tests

**File:** `apps/consumer/src/mode/resolver/caip22_resolver.rs`

Add test for Ethereum Sepolia NFT resolution:
```rust
#[tokio::test]
async fn test_fetch_real_nft_ethereum_sepolia() {
    // Test token 3265 on Ethereum Sepolia
}
```

**File:** `apps/consumer/src/mode/metadata.rs`

Add test for fallback label:
```rust
#[test]
fn test_caip22_fallback_label_uses_token_id() {
    let metadata = AtomMetadata::caip22(None, None, Some("3265".to_string()));
    assert_eq!(metadata.label, "3265");
}
```

## Files to Modify

1. `apps/consumer/src/config.rs` - Add `polygon_amoy_rpc_url` field
2. `apps/consumer/src/mode/resolver/caip22_resolver.rs` - Add Polygon Amoy chain mapping
3. `apps/consumer/src/mode/metadata.rs` - Fix fallback label, update `caip22()` signature
4. `docker/docker-compose-apps.yml` - Add POLYGON_AMOY_RPC_URL environment variable
5. `scripts/start.sh` - Add export for POLYGON_AMOY_RPC_URL
6. `.env.sample` - Add POLYGON_AMOY_RPC_URL example
7. `integration-tests/src/create-agent.test.ts` - Add Ethereum Sepolia test case

## Testing Plan

1. Run existing CAIP-22 unit tests: `cargo test -p consumer -- caip22`
2. Run integration tests: `pnpm test src/create-agent.test.ts`
3. Verify Ethereum Sepolia token 3265 resolves correctly
4. Verify fallback label shows "3265" instead of "NFT" when resolution fails
5. Verify Polygon Amoy chain is recognized (may not have real NFTs to test)

## Dependencies

- Alchemy or other RPC provider API key for Polygon Amoy
- Real NFT contract on Ethereum Sepolia for testing (0x8004a6090Cd10A7288092483047B097295Fb8847 token 3265)
