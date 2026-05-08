# Secp256k1 Verification Contract

The Secp256k1 Verification contract performs EIP-191 secp256k1 signature verification for cross-chain WarpDrive attestations. It is the cryptographic core of the verification pipeline for processes that bridge with Ethereum and other EVM chains -- it takes an envelope and a set of signatures, recovers the public key from each signature using `secp256k1_recover`, verifies that each recovered key matches a registered signer in the Secp256k1 Security contract, and checks that the cumulative weight of all valid signers meets the required threshold.

The contract is stateless beyond its configuration (admin address and linked Security contract). It does not store any attestation data; it purely validates and returns success or failure. This makes it reusable across different Handler implementations and composable with any contract that needs to verify Vectr attestations.

## When to use this contract

Use the Secp256k1 Verification contract when your WarpDrive process involves cross-chain communication with Ethereum or other EVM-compatible chains. Vectrs sign attestations with secp256k1 keys using Ethereum-format signatures (r||s||v with v=27/28), and this contract recovers the signer using Soroban's `secp256k1_recover` precompile. The EIP-191 message format (`"\x19Ethereum Signed Message:\n32" || keccak256(envelope)`) ensures signatures are compatible with standard EVM tooling.

For Soroban-native processes that don't need EVM compatibility, see the [Ed25519 Verification contract](../ed25519-verification/).

## Source Layout

| File | Purpose |
|------|---------|
| [`src/contract.rs`](./src/contract.rs) | Implements `Secp256k1VerificationInterface` and `WarpDriveInterface` |
| [`src/utils.rs`](./src/utils.rs) | Keccak256 hashing, EIP-191 wrapping, and `secp256k1_recover` glue |
| [`src/storage.rs`](./src/storage.rs) | Admin and linked Security contract address |
| [`src/lib.rs`](./src/lib.rs) | Crate root and module wiring |

## Contract Interactions

**[Secp256k1 Security contract](../secp256k1-security/)** -- Calls the Security contract via the [`Secp256k1SecurityClient`](../../packages/shared/src/interfaces/security.rs) trait to look up signer weights and the required threshold. Uses historical weight queries (`get_signer_weights_at`, `required_weight_at`) when a `reference_block` is provided, ensuring verification uses the signer set that was active when the Vectrs signed.

**[Ethereum Handler contract](../ethereum-handler/)** -- The Handler calls `verify` after decoding the envelope and extracting signatures. The Verification contract validates the signatures and returns success or a typed error. The Handler does not need to understand the cryptographic details.

**Off-chain components** -- Aggregators do not interact with this contract directly. They submit envelopes to the Handler, which delegates to Verification. However, off-chain tooling can call `check_one` to pre-validate a single signature before aggregation, or query `required_weight` and `signer_weight` for monitoring. The [`warpdrive-client`](../../packages/client/) package provides a typed async client (`Secp256k1Verification`) for these calls.

## Signature Verification Flow

1. Keccak256 hash the envelope bytes
2. EIP-191 wrap: `keccak256("\x19Ethereum Signed Message:\n32" || hash)`
3. `secp256k1_recover` from the digest + signature (65-byte r||s||v format, v=27/28)
4. Compress the recovered key and compare to the expected signer pubkey
5. Signatures must be provided in strict ascending order of signer pubkeys (prevents duplicates)
6. Cumulative weight of all valid signers must meet or exceed `required_weight`

## Interface

The full interface is defined in [`Secp256k1VerificationInterface`](../../packages/shared/src/interfaces/verification.rs). Standard admin / upgrade / version methods come from [`WarpDriveInterface`](../../packages/shared/src/interfaces/warpdrive.rs).

### State-Changing Actions

| Function | Description |
|----------|-------------|
| `upgrade(new_wasm_hash, new_version)` | Upgrade the contract WASM. Admin-only. |
| `propose_admin(new_admin)` | Propose a new admin (two-step transfer). Current admin only. |
| `accept_admin()` | Accept a pending admin transfer. Pending admin only. |

### Queries

| Function | Description |
|----------|-------------|
| `verify(envelope, signatures, signer_pubkeys, reference_block)` | Verify that the given signatures over the envelope carry sufficient weight. Signatures and pubkeys must be parallel arrays in strict ascending pubkey order. Returns `Ok(())` on success. |
| `check_one(envelope, signature, signer_pubkey, reference_block)` | Verify a single signature and return the signer's weight. Useful for pre-validation before aggregation. If `reference_block` is `None`, uses current weights. |
| `security_contract() -> Address` | Return the address of the linked Security contract. |
| `required_weight() -> u64` | Return the current required weight from the Security contract. |
| `signer_weight(signer_pubkey) -> u64` | Return the current weight of a signer from the Security contract. |
| `admin() -> Address` | Return the current admin address. |
| `pending_admin() -> Option<Address>` | Return the pending admin, if a transfer is in progress. |
| `version() -> String` | Return the current contract version. |

### Types

Defined in [`packages/shared/src/interfaces/mod.rs`](../../packages/shared/src/interfaces/mod.rs):

- **`CompressedSecpPubKey`** -- `BytesN<33>` -- compressed secp256k1 public key (33 bytes).
- **`SecpSignature`** -- `BytesN<65>` -- ECDSA signature in r||s||v format (65 bytes, v=27/28).

### Errors

All verification errors are returned as typed `VerifyError` variants. Invalid signatures are detected by comparing the recovered public key against the expected signer, so they produce a clean error rather than a panic.

| Code | Name | Description |
|------|------|-------------|
| 301 | `InvalidSignature` | Signature recovery did not produce the expected public key |
| 302 | `SignerNotRegistered` | Recovered signer has zero weight in the Security contract |
| 303 | `InsufficientWeight` | Cumulative signer weight is below the required threshold |
| 304 | `EmptySignatures` | No signatures were provided |
| 305 | `LengthMismatch` | Signatures and signer_pubkeys arrays have different lengths |
| 306 | `SignersNotOrdered` | Signer public keys are not in strict ascending order |
| 307 | `ZeroRequiredWeight` | Required weight is zero (no signers registered or threshold misconfigured) |

## Events

The Verification contract is stateless beyond configuration. Only upgrades emit events.

| Event | Topic | Data Fields | Emitted By |
|-------|-------|-------------|------------|
| `ContractUpgraded` | -- | `version: String` | `upgrade` |
