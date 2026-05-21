# Deploying hodlers-app

Two deploy shapes, both terminating at the same on-chain state:

- **§ B — single-operator dev deploy.** Everything on one host. Quorum is
  1-of-1. Quickest way to see the pipeline end-to-end.
- **§ C — multi-operator production deploy.** N hosts, each running their
  own operator with their own BIP39 mnemonic, ed25519 quorum (e.g. 2-of-3)
  on-chain. What the demo's live deployment actually runs.

Read § A first — both paths depend on the same prerequisites.

---

## A. Prerequisites (shared)

### Host packages

Ubuntu 24.04 (or any Linux with a recent glibc) on each box. Tested on
Hetzner CPX22 (2 vCPU, 4 GB RAM). With 4 GB you'll likely need swap to
`cargo install` Stellar tools; the prebuilt binaries route avoids that.

| Tool | Why | Install |
|---|---|---|
| `curl`, `ca-certificates`, `build-essential`, `pkg-config`, `libssl-dev`, `libudev-dev`, `jq`, `docker.io`, `python3` | shell / build / IPC | `apt update && apt install -y curl ca-certificates build-essential pkg-config libssl-dev libudev-dev jq docker.io python3` |
| Rust 1.95 (rustup) | pinned by `rust-toolchain.toml`; required for component + contract builds | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh -s -- -y --default-toolchain stable --no-modify-path`<br>`. "$HOME/.cargo/env"`<br>`rustup target add wasm32-wasip1 wasm32v1-none`<br>(also: `echo '. "$HOME/.cargo/env"' >> ~/.bashrc` so new SSH sessions inherit `PATH`) |
| `task` (go-task) | Taskfile runner | `sh -c "$(curl --location https://taskfile.dev/install.sh)" -- -d -b /usr/local/bin` |
| `wkg` | fetches the WIT deps for circuit + aggregator | `cargo install --locked wkg` |
| `cargo-component` | builds WASI 0.2 components | **Prefer prebuilt** to avoid OOM on small VMs: <br>`curl -sSL https://github.com/bytecodealliance/cargo-component/releases/download/v0.21.1/cargo-component-x86_64-unknown-linux-gnu.tar.gz \| tar xz -C /tmp && install /tmp/cargo-component-x86_64-unknown-linux-gnu/cargo-component /usr/local/bin/` |
| `stellar` CLI | Soroban contract deploys + key management + RPC simulations | **Prefer prebuilt** for the same reason: <br>`curl -sSL https://github.com/stellar/stellar-cli/releases/download/v26.0.0/stellar-cli-26.0.0-x86_64-unknown-linux-gnu.tar.gz \| tar xz -C /tmp && install /tmp/stellar /usr/local/bin/` |
| `warpdrive` (node binary) | the operator runtime | `cargo install --path ../warpdrive/packages/warpdrive --locked`; see the **upstream patches** note below |
| `warpdrive-cli` | service registration, signer query, upload-component | `cargo install --path ../warpdrive/packages/cli --locked` |
| Pinata account | IPFS pinning of `service.json` (multi-op only) | sign up at `https://app.pinata.cloud`, generate a JWT with `pinFileToIPFS` permission, export as `PINATA_JWT` |

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

### Upstream `warpdrive` patches

The reference `warpdrive` node binary needs three small patches to work
with this demo. Each is independent; the first is required for correctness
(missing CAS means accumulated state is lost under concurrency), the
second is required for the Stellar health check to ever return healthy,
the third is purely diagnostic.

| Patch | Where | Why |
|---|---|---|
| Real CAS in `wasi:keyvalue/atomics::swap` | `packages/engine/src/backend/wasi_keyvalue/atomics.rs` | Upstream `swap` is a stub that just stores the value (last-writer-wins). The circuit's accumulator depends on real CAS semantics. |
| Real Stellar chain health check | `packages/utils/src/health.rs` (`check_stellar_chain_health_query`) | Upstream returns `Err(NotImplemented)` unconditionally so Stellar chains never report healthy. |
| Verbose receive-validation logging | `packages/warpdrive/src/subsystems/aggregator/validate.rs` (`Ed25519` arm) | Optional; logs the exact args sent to `check_one` so signature mismatches are debuggable. |

Either maintain a fork of warpdrive with these patches and install from
your fork, or apply them to a fresh upstream tree before `cargo install`.

### Environment

Each operator box exports the following before `task run-node` / any
deploy task:

| Variable | Set by | Notes |
|---|---|---|
| `DEPLOYER_SECRET` | `stellar keys show <alias>` | Funded testnet S-secret. Used as `--source` for the operator's own `stellar contract deploy` calls AND as the Stellar aggregator credential the node uses to submit `verify_xlm` transactions. |
| `DEPLOYER_ADDRESS` | `stellar keys address <alias>` | G-address matching `DEPLOYER_SECRET`. Required by the warpdrive middleware Docker container. |
| `WARPDRIVE_SIGNING_MNEMONIC` | `stellar keys show <alias> --phrase` | **Must be a BIP39 mnemonic**, not a raw hex secret. The node HD-derives operator keys at non-zero indices for each registered service, which only works against a BIP39 seed. The `stellar` CLI generates one for you when you create a key. |
| `PINATA_JWT` (multi-op admin only) | `https://app.pinata.cloud` | Used by `task publish-service` to pin `service/service.json` to IPFS. |

Persist them by writing a `.env` file at the repo root and `source`-ing
before running tasks:

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

### Hetzner / cloud firewall

For each operator box:

| Port | Protocol | Source | Purpose |
|---|---|---|---|
| 22 | TCP | any | SSH |
| 9000 | TCP | any | libp2p P2P between operators |
| 8000 | TCP | (none, or restricted to a bridge host) | warpdrive node HTTP API; only needed if you're proxying it for a dashboard |
| — | ICMP | any | ping diagnostics |

If you do expose `:8000` to a bridge host, edit `warpdrive.toml` and set
`host = "0.0.0.0"` under `[warpdrive]`, otherwise the node binds to
`127.0.0.1:8000` and even the bridge on the same VPC can't reach it.

---

## B. Single-operator dev deploy

All on one box. ~10 commands. Quorum is 1-of-1; service is registered via
the node's `/dev/services` HTTP endpoint (no IPFS needed).

### 1. Build + deploy on-chain contracts

```bash
cd hodlers-app
task fetch-wit                 # one-time; populates wit-definitions/wit/deps/
task deploy                    # builds all + deploys middleware + hodlers + handler
```

After this completes:
- `out/deploy.json` — middleware-produced manifest with `ed25519_security`, `ed25519_verification`, `project_root`.
- `out/hodlers.json` — your hodlers contract C-address.
- `out/handler.json` — your stellar-handler contract C-address.

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
task wire-service              # build-circuit + build-aggregator + upload + register
```

That runs `upload-component` + `upload-aggregator` against the local node
(produces `out/circuit.digest`, `out/aggregator.digest`), assembles
`service/service.json` referencing those digests + your on-chain manager,
then POSTs to `http://127.0.0.1:8000/dev/services` (skip IPFS for single-op).

The node log should show `Adding service: hodlers`, `services=1, workflows=1, components=2`, and
`StartListeningChain` for both `stellar:pubnet` + `stellar:testnet`.

### 4. Register the operator's signing key on-chain

```bash
task register-signer           # auto-fetches the local node's pubkey + registers
```

Default threshold is 1/1, which is what you want for single-op.

### 5. Verify end-to-end

Trigger a Phoenix XLM-USDC swap on mainnet (any way: via the Phoenix UI
with a small amount, or another bot). Watch the node log — you should see
the 8-event burst, the circuit emit a `payload_size=…` line, the aggregator
sign, and a `verify_xlm` submission to testnet. Then query Hodlers:

```bash
HODLERS=$(jq -r .hodlers out/hodlers.json)
stellar contract invoke --id "$HODLERS" \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015" \
  --source $DEPLOYER_SECRET \
  -- all_points
```

A non-empty array with `(trader, points)` for the trader who just swapped
confirms the round-trip. Points are stored in stroop-equivalent units
matching the swap's XLM amount; sign reflects direction (buy XLM
positive, sell XLM negative).

---

## C. Multi-operator production deploy

Same on-chain contracts as § B, but now N operator VMs cooperate over
libp2p, signatures are aggregated to a configurable quorum threshold
(e.g. 2-of-3), and `service.json` is pinned to IPFS and surfaced via
`project_root.service_uri()` so every operator auto-discovers it.

This walkthrough assumes 3 operators. Generalising to N is trivial.

### 1. Admin laptop — deploy on-chain contracts once

Same as § B step 1:

```bash
cd hodlers-app
task fetch-wit
task deploy
```

Then distribute the three resulting JSON files to every operator so they
all see the same contract addresses:

```bash
for host in op-1 op-2 op-3; do
  scp out/{deploy,hodlers,handler}.json $host:~/hodlers-app/out/
done
```

### 2. Each operator box — clone repo, bootstrap secrets

On every operator (ssh in once each):

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

### 3. Bootstrap operator (call it op-1) — start node first to learn its peer_id

`warpdrive.toml`'s `[warpdrive.p2p.remote]` block on the bootstrap operator
must have `bootstrap_nodes = []`. Then:

```bash
set -a; source .env; set +a
task run-node
```

In the startup log, find a line like:

```
INFO Using P2P identity derived from signing_mnemonic (peer_id: 12D3KooW...)
```

That `12D3KooW…` value is op-1's libp2p PeerId. It's deterministic — derived
from `WARPDRIVE_SIGNING_MNEMONIC` at HD path `m/44'/60'/0'/0/0` — so once
the mnemonic is set, the peer_id is stable across restarts.

Keep the node running.

### 4. Other operators — point at op-1's multiaddr, start

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
- `Identified peer …: protocol=/warpdrive/1.0.0`
- `Kademlia bootstrap complete`

### 5. Each operator — build + upload components locally

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

All three operators must produce identical digests — they do, because the
wasm builds are deterministic.

### 6. Admin — assemble service.json

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

### 7. Admin — pin service.json to IPFS + point project_root at it

```bash
export PINATA_JWT=...                # https://app.pinata.cloud
task publish-service                 # pin-service + set-service-spec
```

`out/service.cid` records the IPFS CID; the middleware Docker container
writes `ipfs://<CID>` to `project_root.service_uri()` on testnet.

### 8. Each operator — tell its node to start watching project_root

```bash
task register-manager
```

Each node fetches `project_root.service_uri()`, pulls the spec from
Pinata's gateway via the `ipfs_gateway` in `warpdrive.toml`, and registers
the service in its local dispatcher. Watch the node log for
`Adding service: hodlers` + `services=1, workflows=1`.

### 9. Each operator — fetch its operator signer pubkey, send to admin

```bash
task fetch-signer
cat out/signer.pubkey      # 64 hex chars (32-byte ed25519)
```

Each operator sends its hex pubkey to the admin.

### 10. Admin — register all three operators + set the quorum threshold

```bash
task register-signer SIGNER_PUBKEY=<op-1 pubkey hex>
task register-signer SIGNER_PUBKEY=<op-2 pubkey hex>
task register-signer SIGNER_PUBKEY=<op-3 pubkey hex>

THRESHOLD_NUM=2 THRESHOLD_DEN=3 task set-threshold
```

(`register-signer` resets threshold to its default of 1/1 on each call,
which is harmless during the initial roll-out; `set-threshold` flips it to
2/3 as the last step.)

### 11. Verify

Trigger a Phoenix swap on mainnet. Watch all three operator logs:

1. All three see the 8-event burst.
2. All three accumulate via CAS, emit the `payload_size=…` line, sign with
   their own ed25519 key, and broadcast over libp2p.
3. Each operator collects sigs until quorum (2-of-3) is reached.
4. All three try to submit `verify_xlm` to the handler. Exactly one wins
   the simulation race; the other two see `EventAlreadySeen` (the
   handler's per-`event_id` dedup) and back off.
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

## D. Operational notes

- **Redundant submission is by design.** When quorum is reached, every
  operator independently calls `verify_xlm`. The on-chain handler dedupes
  by `event_id`, so the redundancy costs N−1 cheap simulations + at most a
  tiny tx fee on one of them. The upside is no single-point-of-failure
  submitter, no leader election protocol needed on top of libp2p.
- **State TTL.** The circuit's accumulator never garbage-collects
  finalised `SwapState` entries (they stay around as `finalized: true`
  tombstones to prevent re-emission). Fine at Phoenix's swap rate; a
  long-running production deployment should add a janitor task.
- **HTTP exposure.** `dev_endpoints_enabled = true` is convenient for the
  demo (debug endpoints, log streaming) but exposes mutating POST routes
  alongside read-only GETs. Don't expose `:8000` publicly — keep it
  internal or front it with a read-only proxy.
- **node-data persistence.** The node persists the service registry
  under `out/node-data/`. If you redeploy the on-chain `project_root` and
  the persistent registry still references the old one, the node will WARN
  about being unable to restore — `rm -rf out/node-data` to wipe.

---

## E. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `task fetch-wit` → `package warpdrive:vectr was not found in component registry` | `wkg` doesn't know how to resolve `warpdrive` namespace | Write `~/.config/wasm-pkg/config.toml` with `default_registry = "wa.dev"` and `[namespace_registries] warpdrive = "warg.wa.dev"` (see § A). |
| `cargo install …` killed with SIGKILL | OOM on a small VM | Add a 4 GB swap file (`fallocate -l 4G /swapfile && chmod 600 /swapfile && mkswap /swapfile && swapon /swapfile`), or use the prebuilt binaries listed in § A. |
| Node logs `[stellar:pubnet] NotImplemented` health check | Building from an unpatched warpdrive tree | Apply the `health.rs` patch from § A "Upstream patches"; rebuild + reinstall the node. |
| `task register-signer` errors with `connection refused on 127.0.0.1:8000` | The local node isn't running and you didn't pass `SIGNER_PUBKEY` | Either start `task run-node` first (the task will auto-fetch the local signer), or pass `SIGNER_PUBKEY=<hex>` explicitly (multi-op flow). |
| Op-A can't reach Op-B `:8000` (Connection refused) | Op-B's warpdrive bound to `127.0.0.1:8000` | Add `host = "0.0.0.0"` under `[warpdrive]` in Op-B's `warpdrive.toml`; restart the node. |
| Op-A can't reach Op-B `:8000` (timeout) | Cloud firewall blocking | Allow inbound TCP 8000 from Op-A's IP on Op-B's firewall (or keep `:8000` internal and front it with a proxy on Op-B itself). |
| Node logs `Failed to restore service for Stellar { … }: Failed to fetch service: Invalid IPFS CID in authority: REPLACE_ME` | Stale persistent registry from a previous deploy | `rm -rf out/node-data` then restart the node. |
| Component executes but `Component execution produced no result` for all 8 events; `dev/kv` shows only 2-3 of the 5 relevant fields populated | Running an unpatched warpdrive (no real CAS in `wasi:keyvalue/atomics::swap`) | Apply the `atomics.rs` patch from § A "Upstream patches"; rebuild + reinstall the node. |
| `verify_xlm` reverts with `EventAlreadySeen` | Expected when a peer already submitted | Not an error; the handler's dedup is doing its job. |
