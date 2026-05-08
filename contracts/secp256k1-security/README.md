# Secp256k1 Security Contract

The Secp256k1 Security contract is the Proof-of-Authority signer registry for cross-chain WarpDrive processes that bridge with Ethereum and other EVM chains. It uses compressed secp256k1 public keys (33 bytes), the same curve and key format used by Ethereum, enabling Vectr operators to use a single key pair for both EVM signing and Stellar attestation. It maintains a set of trusted operator public keys, each assigned a weight, and defines the threshold that must be met for an attestation to be considered valid. The admin manages the signer set -- adding operators when they join the project and removing them when they leave. The verification threshold is expressed as a fraction (`numerator / denominator`) of the total weight, allowing governance to tune the required quorum (e.g., 2/3 of total weight).

Signer weights are stored using a checkpoint system, which enables point-in-time lookups. This means the Verification contract can query "what was this signer's weight at ledger N?" -- critical for ensuring that attestation signatures are validated against the signer set that was active when the Vectrs actually produced those attestations, not the current set which may have changed since then.

## When to use this contract

Use the Secp256k1 Security contract when your WarpDrive process involves cross-chain communication with Ethereum or other EVM-compatible chains. Vectrs in this configuration sign attestations with secp256k1 keys using EIP-191 message formatting, and the corresponding Secp256k1 Verification contract recovers the signer from the signature using `secp256k1_recover`. This is the right choice for any bridge, cross-chain oracle, or multi-chain dApp where the same operator keys need to be valid on both Stellar and an EVM chain.

For Soroban-native processes that don't need EVM compatibility, see the [Ed25519 Security contract](../ed25519-security/).

## Source Layout

| File | Purpose |
|------|---------|
| [`src/contract.rs`](./src/contract.rs) | Implements `Secp256k1SecurityInterface` and `WarpDriveInterface` |
| [`src/storage.rs`](./src/storage.rs) | Persistent signer set, threshold storage, and `CheckpointStore` impl over signer weights |
| [`src/lib.rs`](./src/lib.rs) | Crate root and module wiring |

The historical-weight machinery (binary search, same-ledger coalescing, pruning) is provided by the [`checkpoint`](../../packages/shared/src/checkpoint.rs) module in `warpdrive-shared`; this contract only supplies the storage backend.

## Contract Interactions

**[Secp256k1 Verification contract](../secp256k1-verification/)** -- The Verification contract calls into this contract via the [`Secp256k1SecurityClient`](../../packages/shared/src/interfaces/security.rs) trait to look up signer weights (both current and historical) and to compute the required weight threshold. These cross-contract calls happen during every signature verification flow.

**Off-chain components** -- Project governance (a multisig, DAO, or single admin) manages the signer set through this contract. When new Vectr operators are onboarded, their compressed secp256k1 public keys are registered here. The `list_signers` query allows off-chain tooling to display the current operator set. Vectrs themselves do not interact with this contract directly -- they only need their own signing keys. The [`warpdrive-client`](../../packages/client/) package provides a typed async client (`Secp256k1Security`) for governance tooling.

**Handler contract** -- The [Ethereum Handler](../ethereum-handler/) does not call this contract directly. All Security queries flow through the Verification contract.

## Interface

The full interface is defined in [`Secp256k1SecurityInterface`](../../packages/shared/src/interfaces/security.rs). Standard admin / upgrade / version methods come from [`WarpDriveInterface`](../../packages/shared/src/interfaces/warpdrive.rs).

### State-Changing Actions

| Function | Description |
|----------|-------------|
| `add_signer(key, weight)` | Add a signer or update their weight. Weight must be non-zero. Admin-only. Emits `SignerAdded`. |
| `remove_signer(key)` | Remove a signer from the registry. Admin-only. Emits `SignerRemoved`. |
| `set_threshold(numerator, denominator)` | Set the verification threshold as a fraction. Numerator must be <= denominator, both non-zero. Admin-only. Emits `ThresholdSet`. |
| `upgrade(new_wasm_hash, new_version)` | Upgrade the contract WASM. Admin-only. |
| `propose_admin(new_admin)` | Propose a new admin (two-step transfer). Current admin only. |
| `accept_admin()` | Accept a pending admin transfer. Pending admin only. |

### Queries

| Function | Description |
|----------|-------------|
| `get_signer_weight(key) -> u64` | Return the current weight of a signer (0 if not registered). |
| `get_signer_weight_at(key, reference_block) -> u64` | Return the weight of a signer at a specific ledger sequence. |
| `get_signer_weights(keys) -> Vec<u64>` | Batch lookup of current weights for multiple signers. |
| `get_signer_weights_at(keys, reference_block) -> Vec<u64>` | Batch lookup of weights at a specific ledger sequence. |
| `get_total_weight() -> u64` | Return the sum of all current signer weights. |
| `get_total_weight_at(reference_block) -> u64` | Return the total weight at a specific ledger sequence. |
| `required_weight() -> u64` | Return the current required weight (`total_weight * numerator / denominator`). |
| `required_weight_at(reference_block) -> u64` | Return the required weight at a specific ledger sequence. |
| `list_signers() -> Vec<SignerInfo>` | Return all registered signers and their weights. |
| `threshold_numerator() -> u64` | Return the threshold numerator. |
| `threshold_denominator() -> u64` | Return the threshold denominator. |
| `admin() -> Address` | Return the current admin address. |
| `pending_admin() -> Option<Address>` | Return the pending admin, if a transfer is in progress. |
| `version() -> String` | Return the current contract version. |

### Types

Defined in [`packages/shared/src/interfaces/security.rs`](../../packages/shared/src/interfaces/security.rs) (with `CompressedSecpPubKey` aliased in [`mod.rs`](../../packages/shared/src/interfaces/mod.rs)):

- **`CompressedSecpPubKey`** -- `BytesN<33>` -- compressed secp256k1 public key (33 bytes).
- **`SignerInfo`** -- `{ key: CompressedSecpPubKey, weight: u64 }` -- a signer and their weight.

### Errors

| Code | Name | Description |
|------|------|-------------|
| 201 | `ZeroDenominator` | Threshold denominator cannot be zero |
| 202 | `NumeratorExceedsDenominator` | Numerator must be <= denominator |
| 203 | `ZeroNumerator` | Threshold numerator cannot be zero |
| 204 | `ZeroWeight` | Signer weight must be non-zero |

## Events

All state-changing operations emit events for off-chain indexing and monitoring.

| Event | Topic | Data Fields | Emitted By |
|-------|-------|-------------|------------|
| `SignerAdded` | `key: BytesN<33>` | `weight: u64` | `add_signer` |
| `SignerRemoved` | `key: BytesN<33>` | -- | `remove_signer` |
| `ThresholdSet` | -- | `numerator: u64`, `denominator: u64` | `set_threshold` |
| `Upgraded` | -- | `version: String` | `upgrade` |

### SignerAdded

Emitted when a signer is added or their weight is updated.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `key` | `BytesN<33>` | yes | Compressed secp256k1 public key |
| `weight` | `u64` | no | New signer weight |

### SignerRemoved

Emitted when a signer is removed.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `key` | `BytesN<33>` | yes | Compressed secp256k1 public key |

### ThresholdSet

Emitted when the verification threshold is changed.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `numerator` | `u64` | no | Threshold numerator |
| `denominator` | `u64` | no | Threshold denominator |

### Upgraded

Emitted when the contract WASM is upgraded.

| Field | Type | Topic | Description |
|-------|------|-------|-------------|
| `version` | `String` | no | New contract version |
