# Resolver Consumer: Changes & Updates

## Overview

This document covers two sets of changes:

1. **ENS Resolver** — migrated from the legacy ENSRegistry/ENSName contract pattern to the ENS Universal Resolver (ENSIP-23), mirroring upstream PR #217.
2. **Integration Tests** — added automatic `.env` loading so scripts can be run directly without manually sourcing environment variables.

The **TNS Resolver** was not changed. It remains fully intact with its existing priority-over-ENS logic.

---

## 1. ENS Resolver Migration (ENSIP-23 Universal Resolver)

### Why

The old implementation did a two-step manual lookup:
1. Call `ENSRegistry.resolver(node)` to find the resolver contract for the address.
2. Call `ENSName.name(node)` on that resolver to get the primary name.

This pattern does not support L2 primary names (Base, Optimism, Linea) or offchain resolvers, and requires multiple RPC calls. The ENS Universal Resolver (ENSIP-23) handles all of this in a single call, with CCIP-Read handled transparently by the provider.

### Files Changed

#### `apps/consumer/src/main.rs`

Replaced the two old sol! macro definitions:

```rust
// REMOVED
sol! { interface ENSRegistry { function resolver(bytes32 node) ... } }
sol! { interface ENSName { function name(bytes32 node) ... } }
```

With the Universal Resolver interface:

```rust
// ADDED
sol! {
    interface UniversalResolver {
        function reverse(bytes lookupAddress, uint256 coinType)
            external view returns (string primary, address resolver, address reverseResolver);
    }
}
```

#### `apps/consumer/src/mode/resolver/ens_resolver.rs`

Complete rewrite. Key differences:

| Old | New |
|-----|-----|
| Took `mainnet_client: &ENSRegistryInstance` | Takes `universal_resolver: &UniversalResolverInstance` |
| Two-step lookup (registry → resolver → name) | Single call: `universal_resolver.reverse(addr_bytes, coin_type)` |
| Only resolved L1 primary names | Resolves L1, L2 (Base/Optimism/Linea via CCIP-Read), and offchain names |
| Errors propagated to caller | Non-fatal: logs a warning and returns `Ok(None)` |

The new `Ens::get_ens_name` ABI-encodes the address as a 20-byte `bytes` argument and uses coin type `60` (ETH per SLIP-44) as required by the Universal Resolver's `reverse()` function.

#### `apps/consumer/src/mode/types.rs`

- `ResolverConsumerContext.mainnet_client` replaced with `universal_resolver: Arc<UniversalResolverInstance<DynProvider, Ethereum>>`
- `build_ens_client()` replaced with `build_universal_resolver_client(rpc_url, contract_address)`
- `create_resolver_consumer()` now builds the Universal Resolver client using `RPC_URL_MAINNET` and an optional `UNIVERSAL_RESOLVER_ADDRESS` env var (defaults to the canonical ENSIP-23 deployment `0xce01f8eee7E479C928F8919abD53E553a36CeF67` if not set)
- `tns_client` field and `build_tns_client()` kept unchanged

#### `apps/consumer/src/config.rs`

Added:

```rust
pub universal_resolver_address: Option<String>,
```

If not set, the code defaults to the canonical Universal Resolver address on Ethereum mainnet.

---

## 2. TNS Resolver (Unchanged)

The TNS resolver (`apps/consumer/src/mode/resolver/tns_resolver.rs`) was not modified. It is entirely local work not present in the upstream.

### Resolution Priority

The `process_account` function in `apps/consumer/src/mode/resolver/types.rs` resolves names in this order:

1. **TNS first** — calls `Tns::reverse_resolve(address, &tns_client)`
   - If a name is found: use it, send avatar to the IPFS upload consumer, stop.
2. **ENS fallback** — calls `Ens::get_ens(address, resolver_consumer_context)`
   - Uses the Universal Resolver via `universal_resolver` field.
   - If a name is found: fetch avatar from `metadata.ens.domains`, send to IPFS upload consumer.
3. **No name found** — logs a debug message, no metadata update.

TNS errors are non-fatal and fall through to ENS automatically.

### TNS Constants

| Constant | Value |
|----------|-------|
| `TNS_REGISTRY_ADDRESS` | `0x3220B4EDbA3a1661F02f1D8D241DBF55EDcDa09e` |
| `INTUITION_RPC_URL` | `https://intuition.calderachain.xyz` |

---

## 3. Integration Tests: Automatic `.env` Loading

### Problem

Running scripts directly with `pnpm exec tsx src/*.script.ts` failed with:

```
Error: VITE_INTUITION_CONTRACT_ADDRESS is not set
```

`tsx` does not automatically load `.env` files into `process.env`. The root `.env` at the repo root had the required values, but they were never read.

### Fix

**`integration-tests/package.json`** — added `dotenv` as a dependency.

**`integration-tests/src/setup/constants.ts`** — added dotenv config at the top of the file, which is imported by all scripts:

```typescript
import dotenv from 'dotenv'
import { resolve } from 'path'
import { fileURLToPath } from 'url'

dotenv.config({ path: resolve(fileURLToPath(import.meta.url), '../../../../.env') })
```

This resolves to the root `.env` (`intuition-rs/.env`) regardless of which directory the script is run from. Since `constants.ts` is imported by every script via `utils.ts`, all scripts pick this up automatically without any changes to individual script files.

---

## Environment Variables Reference

| Variable | Used By | Default | Notes |
|----------|---------|---------|-------|
| `RPC_URL_MAINNET` | Universal Resolver | — | Required. Ethereum mainnet RPC. |
| `UNIVERSAL_RESOLVER_ADDRESS` | Universal Resolver | `0xce01f8eee7E479C928F8919abD53E553a36CeF67` | Optional. Canonical ENSIP-23 address. |
| `ENS_CONTRACT_ADDRESS` | — | — | No longer used. Can be removed from docker-compose. |
| `VITE_INTUITION_CONTRACT_ADDRESS` | Integration tests | — | Loaded from root `.env` via dotenv. |
