# Deploying hodlers-app

Two deploy shapes, both terminating at the same on-chain state:

- **Single-operator dev deploy.** Everything on one host. Quorum is 1-of-1.
  Quickest way to see the pipeline end-to-end.
- **Multi-operator production deploy.** N hosts, each running their own
  operator with their own BIP39 mnemonic, ed25519 quorum (e.g. 2-of-3)
  on-chain.

Read **Prerequisites** first; both paths depend on the same setup.

---

## Prerequisites

### Host packages

Linux with a recent glibc. The following tools must be on `PATH`:

| Tool                    | Why                                                                                              |
| ----------------------- | ------------------------------------------------------------------------------------------------ |
| `curl`, `jq`, `python3` | Shell / build / IPC                                                                              |
| `docker`                | Runs the [`warpdrive-stellar-middleware`](https://github.com/warp-driver/warpdrive-stellar-middleware) container                                       |
| Rust 1.95 (via rustup)  | Pinned by `rust-toolchain.toml`; required for component + contract builds                        |
| `task` (go-task)        | Runs `Taskfile.yml` targets                                                                      |
| `wkg`                   | Fetches the WIT deps for the WASI components                                                     |
| `cargo-component`       | Builds WASI 0.2 components (`circuit`, `aggregator`)                                             |
| `stellar` CLI           | Soroban contract deploys, key management, RPC simulations                                        |
| `warpdrive`             | The operator runtime, built from [`warp-driver/warpdrive/packages/warpdrive`](https://github.com/warp-driver/warpdrive/tree/main/packages/warpdrive) (see Upstream patches)                       |
| `warpdrive-cli`         | Service registration, signer queries, component uploads, built from [`warp-driver/warpdrive/packages/cli`](https://github.com/warp-driver/warpdrive/tree/main/packages/cli)                       |
| Pinata account          | IPFS-pinning of `service.json` (multi-operator only); sign up at `https://app.pinata.cloud`      |

Rust setup once the toolchain is installed:

```bash
rustup target add wasm32-wasip1 wasm32v1-none
```

After `wkg` is installed, point it at the registry so it can resolve the
`warpdrive` namespace WIT packages:

```bash
mkdir -p ~/.config/wasm-pkg
cat > ~/.config/wasm-pkg/config.toml <<'EOF'
default_registry = "wa.dev"

[namespace_registries]
warpdrive = "warg.wa.dev"
EOF
```

### Upstream warpdrive patches

The reference [`warpdrive`](https://github.com/warp-driver/warpdrive) node
binary needs three small patches to work with this demo:

| Patch                                               | Where (in [`warp-driver/warpdrive`](https://github.com/warp-driver/warpdrive))                                                                                                          | Why                                                                                                                                       |
| --------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Real CAS in `wasi:keyvalue/atomics::swap`           | [`packages/engine/src/backend/wasi_keyvalue/atomics.rs`](https://github.com/warp-driver/warpdrive/blob/main/packages/engine/src/backend/wasi_keyvalue/atomics.rs)                       | Upstream `swap` is a stub that just stores the value (last-writer-wins). The circuit's accumulator depends on real CAS semantics.         |
| Real Stellar chain health check                     | [`packages/utils/src/health.rs`](https://github.com/warp-driver/warpdrive/blob/main/packages/utils/src/health.rs) (`check_stellar_chain_health_query`)                                 | Upstream returns `Err(NotImplemented)` unconditionally so Stellar chains never report healthy.                                            |
| Verbose receive-validation logging (optional)       | [`packages/warpdrive/src/subsystems/aggregator/validate.rs`](https://github.com/warp-driver/warpdrive/blob/main/packages/warpdrive/src/subsystems/aggregator/validate.rs) (Ed25519 arm) | Logs the exact args sent to `check_one` so signature mismatches are debuggable.                                                           |

Maintain a fork of [`warp-driver/warpdrive`](https://github.com/warp-driver/warpdrive)
with these applied, or apply to a fresh upstream tree before building the
node binary.

### Environment variables

Each operator box exports the following before running any task:

| Variable                     | Set by                                | Notes                                                                                                                                                       |
| ---------------------------- | ------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DEPLOYER_SECRET`            | `stellar keys show <alias>`           | Funded testnet S-secret. Used as `--source` for the operator's own contract deploys AND as the Stellar aggregator credential the node uses to submit txs.   |
| `DEPLOYER_ADDRESS`           | `stellar keys address <alias>`        | G-address matching `DEPLOYER_SECRET`. Required by the warpdrive middleware Docker container.                                                                |
| `WARPDRIVE_SIGNING_MNEMONIC` | `stellar keys show <alias> --phrase`  | Must be a BIP39 mnemonic, not a raw hex secret. The node HD-derives operator keys at non-zero indices, which only works against a BIP39 seed.               |
| `PINATA_JWT`                 | `https://app.pinata.cloud`            | Multi-op admin only. Used by `task publish-service` to pin `service/service.json` to IPFS.                                                                  |

Persist them in a `.env` at the repo root and source before each session:

```bash
cd hodlers-app
stellar keys generate hodlers-deployer --fund --network testnet
stellar keys generate warpdrive-operator
cat > .env <<EOF
DEPLOYER_SECRET=$(stellar keys show hodlers-deployer)
DEPLOYER_ADDRESS=$(stellar keys address hodlers-deployer)
WARPDRIVE_SIGNING_MNEMONIC="$(stellar keys show warpdrive-operator --phrase)"
EOF
set -a; source .env; set +a
```

### Firewall (multi-operator only)

For each operator box:

| Port | Protocol | Source                                            | Purpose                            |
| ---- | -------- | ------------------------------------------------- | ---------------------------------- |
| 22   | TCP      | any                                               | SSH                                |
| 9000 | TCP      | any                                               | libp2p P2P between operators       |
| 8000 | TCP      | restricted (or kept internal)                     | warpdrive node HTTP API; only open if a separate host needs to read it (e.g. a dashboard) |
| -    | ICMP     | any                                               | Ping diagnostics                   |

If you expose `:8000` to any host other than `127.0.0.1`, edit
`warpdrive.toml` and set `host = "0.0.0.0"` under `[warpdrive]`; the node
otherwise binds only to localhost.

---

## Single-operator dev deploy

All on one box. ~10 commands. Quorum is 1-of-1; service is registered via
the node's `/dev/services` HTTP endpoint (no IPFS needed).

### 1. Build + deploy on-chain contracts

```bash
cd hodlers-app
task fetch-wit                 # one-time; populates wit-definitions/wit/deps/
task deploy                    # builds all + deploys middleware + hodlers + handler
```

After this completes:
- `out/deploy.json` - middleware-produced manifest with `ed25519_security`, `ed25519_verification`, `project_root`.
- `out/hodlers.json` - your hodlers contract C-address.
- `out/handler.json` - your stellar-handler contract C-address.

### 2. Start the operator node

In a second terminal:

```bash
cd hodlers-app
set -a; source .env; set +a
task run-node
```

Wait for both lines:

```
INFO Stellar chain [stellar:pubnet]  is healthy
INFO Stellar chain [stellar:testnet] is healthy
```

Leave the node running.

### 3. Upload components + register the service

Back in the first terminal:

```bash
task wire-service
```

That runs `upload-component` + `upload-aggregator` against the local node
(produces `out/circuit.digest`, `out/aggregator.digest`), assembles
`service/service.json` referencing those digests + your on-chain manager,
then POSTs to `http://127.0.0.1:8000/dev/services`.

The node log should show `Adding service: hodlers`, `services=1, workflows=1, components=2`,
and `StartListeningChain` for both `stellar:pubnet` and `stellar:testnet`.

### 4. Register the operator's signing key on-chain

```bash
task register-signer
```

The task auto-fetches the local node's pubkey and registers it at the
default threshold 1/1, which is what you want for single-op.

### 5. Verify end-to-end

Trigger a Phoenix XLM-USDC swap on mainnet. Watch the node log for the
8-event burst, the circuit emit (`payload_size=...`), the aggregator sign,
and a `verify_xlm` submission to testnet. Then query Hodlers:

```bash
HODLERS=$(jq -r .hodlers out/hodlers.json)
stellar contract invoke --id "$HODLERS" \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015" \
  --source $DEPLOYER_SECRET \
  -- all_points
```

A non-empty array with `(trader, points)` for the trader who just swapped
confirms the round-trip.

---

## Multi-operator production deploy

Same on-chain contracts as the single-op path, but now N operator hosts
cooperate over libp2p, signatures are aggregated to a configurable quorum
threshold (e.g. 2-of-3), and `service.json` is pinned to IPFS and surfaced
via `project_root.service_uri()` so every operator auto-discovers it.

The walkthrough below assumes 3 operators. Generalising to N is trivial.

### 1. Admin box - deploy on-chain contracts once

Same as the single-op step 1:

```bash
cd hodlers-app
task fetch-wit
task deploy
```

Distribute the three resulting JSON files to every operator so they all
see the same contract addresses:

```bash
for host in op-1 op-2 op-3; do
  scp out/{deploy,hodlers,handler}.json $host:~/hodlers-app/out/
done
```

### 2. Each operator box - clone repo, bootstrap secrets

On every operator:

```bash
git clone <repo-url> hodlers-app
cd hodlers-app
task fetch-wit

stellar keys generate hodlers-deployer --fund --network testnet
stellar keys generate warpdrive-operator
cat > .env <<EOF
DEPLOYER_SECRET=$(stellar keys show hodlers-deployer)
DEPLOYER_ADDRESS=$(stellar keys address hodlers-deployer)
WARPDRIVE_SIGNING_MNEMONIC="$(stellar keys show warpdrive-operator --phrase)"
EOF
```

Edit `warpdrive.toml` and set `host = "0.0.0.0"` under `[warpdrive]` if any
host other than `127.0.0.1` needs to reach `:8000`.

### 3. Bootstrap operator (op-1) - start node to learn its peer_id

On the bootstrap operator, edit `warpdrive.toml` so the
`[warpdrive.p2p.remote]` block has `bootstrap_nodes = []`. Then:

```bash
set -a; source .env; set +a
task run-node
```

Find the log line:

```
INFO Using P2P identity derived from signing_mnemonic (peer_id: 12D3KooW...)
```

That value is op-1's libp2p PeerId. It is deterministic (derived from
`WARPDRIVE_SIGNING_MNEMONIC` at HD path `m/44'/60'/0'/0/0`) so it stays
the same across restarts.

Keep the node running.

### 4. Other operators - point at op-1's multiaddr, start

On op-2 and op-3, edit `warpdrive.toml` so the `[warpdrive.p2p.remote]`
block contains:

```toml
[warpdrive.p2p.remote]
listen_port = 9000
bootstrap_nodes = [
  "/ip4/<op-1-public-ip>/tcp/9000/p2p/<op-1-peer-id>",
]
```

Then start each:

```bash
set -a; source .env; set +a
task run-node
```

Confirm in op-2 / op-3 logs:
- `Connection established with <op-1 peer_id>`
- `Identified peer ...: protocol=/warpdrive/1.0.0`
- `Kademlia bootstrap complete`

### 5. Each operator - build + upload components locally

```bash
# In a second ssh session on each operator
cd hodlers-app
set -a; source .env; set +a
task build-circuit
task build-aggregator
task upload-component
task upload-aggregator
cat out/circuit.digest out/aggregator.digest
```

All three operators must produce identical digests - they do, because the
wasm builds are deterministic.

### 6. Admin - assemble service.json

Pull the digests from any one operator (they all match):

```bash
scp <one-op-host>:~/hodlers-app/out/circuit.digest    out/
scp <one-op-host>:~/hodlers-app/out/aggregator.digest out/
task build-service
```

Inspect `service/service.json` and confirm:
- `manager.stellar.address` is your `project_root` from `out/deploy.json`
- `signature_kind = { algorithm: "ed25519", prefix: "sep53" }`
- both component digests appear

### 7. Admin - pin service.json to IPFS + point project_root at it

```bash
export PINATA_JWT=...
task publish-service
```

`out/service.cid` records the IPFS CID; the middleware Docker container
writes `ipfs://<CID>` to `project_root.service_uri()` on testnet.

### 8. Each operator - tell its node to start watching project_root

```bash
task register-manager
```

Each node fetches `project_root.service_uri()`, pulls the spec from
Pinata's gateway via the `ipfs_gateway` in `warpdrive.toml`, and registers
the service in its local dispatcher. Watch the node log for
`Adding service: hodlers` and `services=1, workflows=1`.

### 9. Each operator - fetch its operator signer pubkey, send to admin

```bash
task fetch-signer
cat out/signer.pubkey      # 64 hex chars (32-byte ed25519)
```

Each operator sends its hex pubkey to the admin.

### 10. Admin - register all three operators + set the quorum threshold

```bash
task register-signer SIGNER_PUBKEY=<op-1 pubkey hex>
task register-signer SIGNER_PUBKEY=<op-2 pubkey hex>
task register-signer SIGNER_PUBKEY=<op-3 pubkey hex>

THRESHOLD_NUM=2 THRESHOLD_DEN=3 task set-threshold
```

`register-signer` resets threshold to its default of 1/1 on each call,
which is harmless during the initial roll-out; `set-threshold` flips it
to 2/3 as the last step.

### 11. Verify

Trigger a Phoenix swap on mainnet. Watch all three operator logs:

1. All three see the 8-event burst.
2. All three accumulate via CAS, emit the `payload_size=...` line, sign
   with their own ed25519 key, and broadcast over libp2p.
3. Each operator collects sigs until quorum (2-of-3) is reached.
4. All three try to submit `verify_xlm` to the handler. Exactly one wins
   the simulation race; the other two see `EventAlreadySeen` and back off.
5. The handler invokes `add_points` on the hodlers contract.

Then on any host:

```bash
HODLERS=$(jq -r .hodlers out/hodlers.json)
stellar contract invoke --id "$HODLERS" \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015" \
  --source $DEPLOYER_SECRET \
  -- all_points
```

You should see the trader's address with their new points total.

---

## Operational notes

- **Redundant submission is by design.** When quorum is reached, every
  operator independently calls `verify_xlm`. The on-chain handler dedupes
  by `event_id`, so the redundancy costs N-1 cheap simulations plus at
  most a tiny tx fee on one of them. No single-point-of-failure submitter,
  no leader election on top of libp2p.
- **State TTL.** The circuit's accumulator never garbage-collects
  finalised `SwapState` entries (they stay as `finalized: true` tombstones
  to prevent re-emission). A long-running production deployment should
  add a janitor task.
- **HTTP exposure.** `dev_endpoints_enabled = true` is convenient for the
  demo but exposes mutating POST routes alongside read-only GETs. Do not
  expose `:8000` publicly without a read-only proxy and auth in front.
- **node-data persistence.** The node persists the service registry under
  `out/node-data/`. If you redeploy the on-chain `project_root` and the
  persistent registry still references the old one, the node WARNs about
  being unable to restore. `rm -rf out/node-data` to wipe.

---

## Troubleshooting

| Symptom                                                                                  | Cause                                                                       | Fix                                                                                                                                                            |
| ---------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `task fetch-wit` -> `package warpdrive:vectr was not found in component registry`        | `wkg` does not know how to resolve the `warpdrive` namespace                | Write `~/.config/wasm-pkg/config.toml` per Prerequisites.                                                                                                      |
| Node logs `[stellar:pubnet] NotImplemented` health check                                 | warpdrive built from an unpatched tree                                      | Apply the `health.rs` patch from Upstream warpdrive patches; rebuild + reinstall the node.                                                                     |
| `task register-signer` errors with `connection refused on 127.0.0.1:8000`                | The local node is not running and `SIGNER_PUBKEY` was not passed            | Either start `task run-node` first (the task auto-fetches the local signer) or pass `SIGNER_PUBKEY=<hex>` explicitly (multi-op flow).                          |
| Operator A cannot reach operator B `:8000` (Connection refused)                          | Operator B's warpdrive bound to `127.0.0.1:8000`                            | Add `host = "0.0.0.0"` under `[warpdrive]` in operator B's `warpdrive.toml`; restart the node.                                                                 |
| Operator A cannot reach operator B `:8000` (timeout)                                     | Cloud firewall blocking                                                     | Allow inbound TCP 8000 from operator A's IP on operator B's firewall (or keep `:8000` internal and front it with a proxy).                                     |
| Node logs `Failed to restore service ... REPLACE_ME`                                     | Stale persistent registry from a previous deploy                            | `rm -rf out/node-data` then restart the node.                                                                                                                  |
| Component executes but `produced no result` for all 8 events; `/dev/kv` shows only 2-3 of 5 relevant fields populated | Running an unpatched warpdrive (no real CAS in `wasi:keyvalue/atomics::swap`) | Apply the `atomics.rs` patch from Upstream warpdrive patches; rebuild + reinstall the node.                                                                    |
| `verify_xlm` reverts with `EventAlreadySeen`                                             | Expected when a peer already submitted                                      | Not an error; the handler's dedup is doing its job.                                                                                                            |
