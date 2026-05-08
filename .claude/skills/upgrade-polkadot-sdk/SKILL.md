---
name: upgrade-polkadot-sdk
description: Use this skill when upgrading encointer-pallets to a new polkadot-sdk release (stable or unstable RC) and re-enabling encointer in the polkadot-fellows runtimes workspace. Triggers include phrases like "upgrade encointer to polkadot-sdk", "bump SDK in encointer pallets", "follow the runtimes SDK upgrade", or any reference to a `polkadot-stableYYMM` / `polkadot-unstableYYMM-rcN` tag in encointer context. Captures sequencing rules and footguns from the encointer/pallets#477 run (May 2026).
version: 0.1.0
---

# Upgrading encointer-pallets to a new Polkadot SDK release

## Purpose
This runbook upgrades `encointer-pallets` to a new `polkadot-sdk` release and re-enables encointer in the polkadot-fellows runtimes workspace.

The runtimes repo is the one that moves first to a new SDK; encointer-pallets follows. While encointer is behind, the runtimes workspace keeps every encointer wiring point commented out behind `TODO @ggwpez encointer` markers. Re-enabling them is mechanical but spread across seven files.

## Path conventions
This skill uses two placeholders for repo paths:
- `<encointer-pallets>` — the encointer-pallets workspace root (where this skill lives, at `<encointer-pallets>/.claude/skills/upgrade-polkadot-sdk/`).
- `<runtimes>` — the polkadot-fellows runtimes workspace root, **assumed to be a sibling of `<encointer-pallets>`** (i.e. `<parent>/encointer-pallets` and `<parent>/runtimes` for some common parent directory).

Resolution at runtime:
- If you're already in the encointer-pallets repo, `<encointer-pallets>` is `pwd` (or whatever `git rev-parse --show-toplevel` reports). `<runtimes>` is `../runtimes` from there.
- The bundled scripts resolve both automatically: `bump-member-versions.sh` walks up from its own location to find `<encointer-pallets>`, and `derive-sdk-versions.sh` defaults its `<runtimes>` argument to the sibling.
- If the layout differs (not siblings, or non-standard names), pass an explicit path to `derive-sdk-versions.sh` and adjust the relative `path = "../encointer-pallets"` strings in the `[patch.crates-io]` block in Phase B accordingly.

## Inputs
Before starting, confirm:
- Target SDK tag, e.g. `polkadot-stable2609` (stable) or `polkadot-unstable2609-rc1` (RC). The runtimes workspace already builds against this tag.
- The `<encointer-pallets>` and `<runtimes>` paths resolve as described under "Path conventions" above. If the sibling assumption doesn't hold, override `<runtimes>` explicitly.
- Whether the operator wants a member-version bump as part of this work (Phase E). Default: yes.

## The sequencing rule (read this first)
**Patches first. Member-version bumps last. `<runtimes>` deps + `[patch.crates-io]` removal happens only after publication.**

Reversing the order silently fails: `<runtimes>/Cargo.toml` already declares tilde requirements like `pallet-encointer-balances = "~22.2.0"`. If encointer member crates are bumped to `22.3.0` before being published, the patches in `<runtimes>` no longer match those tildes and cargo silently falls back to the *old* (still-published) crates, producing duplicate substrate cohorts in the dep graph that don't show up in `cargo check`. The closing check (Phase G) is `cargo tree --duplicates` precisely to catch this.

The phases below enforce the sequence.

## Phase A — bump encointer-pallets `[workspace.dependencies]`

Working dir: `<encointer-pallets>`.

Goal: update every Polkadot SDK / Substrate / Cumulus / xcm dep in the root `Cargo.toml` `[workspace.dependencies]` table to the cohort the runtimes repo is on. **Do not touch any member crate `package.version` here — that's Phase E.**

For **stable** SDK releases:
```bash
cargo install cargo-psvm   # if not already installed
cargo psvm -v <stableYYMM> # bumps every workspace dep
```

For **unstable RCs**: `psvm` doesn't ship RC versions. Derive the table from `<runtimes>`:
```bash
.claude/skills/upgrade-polkadot-sdk/scripts/derive-sdk-versions.sh <runtimes>
```
That script prints, per crate in our `[workspace.dependencies]`:
- the version we currently pin
- the version `<runtimes>/Cargo.toml` pins for the same crate
- a tail section listing crates the runtimes manifest does NOT pin (RPC-side: `sc-rpc`, `sc-rpc-api`, `sp-blockchain`, `sp-rpc`, `pallet-transaction-payment-rpc`, plus the `sp-keystore`/`sp-inherents`/`sp-keyring` dev-deps). For those, apply a `+1 minor` heuristic (e.g. `sc-rpc 51.0.0 → 52.0.0`); cargo check will fail loudly with `error: failed to select a version` if the heuristic version isn't on crates.io, in which case fall back to manually checking the relevant crate page.

After applying:
```bash
rm -f Cargo.lock                         # only if you want a totally fresh resolve
cargo check --workspace                  # regenerates Cargo.lock against new versions
grep -A1 'name = "sp-runtime"' Cargo.lock # sanity: should show the new sp-runtime version
```

### Phase A verification
Run, in order:
```bash
cargo check --workspace
cargo test --workspace
./scripts/run_for_all_no_std_crates.sh check --no-default-features --target=wasm32-unknown-unknown
```

**Anti-pattern**: do NOT run `cargo check --workspace --no-default-features`. It appears to fail with `error[E0433]: failed to resolve: use of unresolved module or unlinked crate std` in pallets that have `#[pallet::genesis_build]` (faucet, democracy were the ones that hit it on #477). Root cause: feature unification with std-only members (`encointer-rpc`, the rpc subdirs) forces `frame-support` to compile with its `std` feature on, which makes the `std_enabled!`-gated genesis_build expansion emit `std::result::Result` into no_std pallet compilations. The canonical no_std verification is the per-crate wasm script.

If the wasm script takes forever and looks like it's checking crates from `target/package/`, run `rm -rf target/package` first — the script's `find . -name Cargo.toml` does not exclude `target/`.

If A surfaces SDK breaks in source files, fix mechanically (renames, signature tweaks). Stop and ask the operator for any change requiring a behavioural decision.

## Phase B — wire encointer back into `<runtimes>`

Working dir: `<runtimes>`.

### B1. Add the `[patch.crates-io]` block
Append to the bottom of `<runtimes>/Cargo.toml`, immediately before `[profile.release]`:

```toml
[patch.crates-io]
encointer-balances-tx-payment                 = { path = "../encointer-pallets/balances-tx-payment" }
encointer-balances-tx-payment-rpc-runtime-api = { path = "../encointer-pallets/balances-tx-payment/rpc/runtime-api" }
encointer-primitives                          = { path = "../encointer-pallets/primitives" }
pallet-encointer-balances                     = { path = "../encointer-pallets/balances" }
pallet-encointer-bazaar                       = { path = "../encointer-pallets/bazaar" }
pallet-encointer-bazaar-rpc-runtime-api       = { path = "../encointer-pallets/bazaar/rpc/runtime-api" }
pallet-encointer-ceremonies                   = { path = "../encointer-pallets/ceremonies" }
pallet-encointer-ceremonies-rpc-runtime-api   = { path = "../encointer-pallets/ceremonies/rpc/runtime-api" }
pallet-encointer-communities                  = { path = "../encointer-pallets/communities" }
pallet-encointer-communities-rpc-runtime-api  = { path = "../encointer-pallets/communities/rpc/runtime-api" }
pallet-encointer-democracy                    = { path = "../encointer-pallets/democracy" }
pallet-encointer-faucet                       = { path = "../encointer-pallets/faucet" }
pallet-encointer-offline-payment              = { path = "../encointer-pallets/offline-payment" }
pallet-encointer-reputation-commitments       = { path = "../encointer-pallets/reputation-commitments" }
pallet-encointer-reputation-rings             = { path = "../encointer-pallets/reputation-rings" }
pallet-encointer-scheduler                    = { path = "../encointer-pallets/scheduler" }
pallet-encointer-treasuries                   = { path = "../encointer-pallets/treasuries" }
pallet-encointer-treasuries-rpc-runtime-api   = { path = "../encointer-pallets/treasuries/rpc/runtime-api" }
```

The relative path assumes `<runtimes>` and `<encointer-pallets>` are siblings. Adjust if not.

### B2. Uncomment every `TODO @ggwpez encointer` marker
The runtimes workspace keeps encointer wiring behind two comment shapes:
- single-line: `# TODO @ggwpez encointer-...` or `// TODO @ggwpez encointer-...`
- block: `/* TODO @ggwpez ... */`

Find them all:
```bash
rg 'TODO @ggwpez.*[Ee]ncointer' <runtimes>
```

On the May 2026 run this matched 30 sites across these files (line numbers will drift; rely on the grep):
- `Cargo.toml` — 5 workspace-dep lines (`encointer-balances-tx-payment`, …, `encointer-primitives`), 15 pallet-encointer-* workspace-dep lines, 3 workspace `members` entries (the encointer runtime, the emulated chain, the integration-tests dir).
- `chain-spec-generator/Cargo.toml` — 5 lines: the `encointer-kusama-runtime` workspace dep, plus its `runtime-benchmarks` and `on-chain-release-build` feature entries, plus the `encointer-kusama` feature definition and its inclusion in `all-kusama`.
- `chain-spec-generator/src/main.rs` — one block comment around the `("encointer-kusama-local", ...)` chain-spec match arm.
- `chain-spec-generator/src/common.rs` — one line comment in the `use crate::system_parachains_specs::{...}` import (`EncointerKusamaChainSpec`) and one block comment around the `x.starts_with("encointer-kusama")` arm.
- `chain-spec-generator/src/system_parachains_specs.rs` — one line comment for the `pub type EncointerKusamaChainSpec = ...` alias and one block comment around the `encointer_kusama_local_testnet_config` function.
- `integration-tests/emulated/networks/kusama-system/Cargo.toml` — 2 lines: the `encointer-kusama-emulated-chain` workspace dep and its `runtime-benchmarks` feature entry.
- `integration-tests/emulated/networks/kusama-system/src/lib.rs` — 4 lines: `pub use encointer_kusama_emulated_chain;`, `use encointer_kusama_emulated_chain::EncointerKusama;`, `EncointerKusama,` in `decl_test_networks!`, and `EncointerKusamaPara { sender: ALICE, receiver: BOB }` in `decl_test_sender_receiver_accounts_parameter_types!`.

For each match, strip the comment marker and keep the rest of the line/block intact. **Do not change the version strings** in the workspace deps — leave the existing `~22.x.0` requirements alone; the patches in B1 are what makes them resolve.

### B3. The trailing-comma footgun
In `integration-tests/emulated/networks/kusama-system/src/lib.rs`, the `decl_test_sender_receiver_accounts_parameter_types!` macro lists items WITHOUT a trailing comma on the last one. Before encointer was disabled, `CoretimeKusamaPara` was the last entry (no trailing comma). Re-adding `EncointerKusamaPara` after it requires adding a comma to `CoretimeKusamaPara`:

```rust
// before                                              // after
CoretimeKusamaPara { sender: ALICE, receiver: BOB }    CoretimeKusamaPara { sender: ALICE, receiver: BOB },
// (no other line)                                     EncointerKusamaPara { sender: ALICE, receiver: BOB }
```

(The `decl_test_networks!` macro above already has trailing commas on every line, so re-adding `EncointerKusama,` there needs no adjustment.)

### B4. Sweep
```bash
rg 'TODO @ggwpez.*[Ee]ncointer' <runtimes>
```
Must return zero hits. If anything is left, you missed a marker.

```bash
cd <runtimes> && cargo metadata --format-version 1 > /dev/null
```
Should exit 0 — confirms the workspace resolves cleanly with the patches.

## Phase C — verify the encointer-kusama runtime

### C0. Toolchain prep (one-time per machine)
`substrate-wasm-builder` (the build script that compiles the runtime to wasm) needs `rust-src` on both nightly and the active stable toolchain:
```bash
rustup component add rust-src --toolchain nightly
rustup component add rust-src --toolchain 1.93.0     # or whatever stable rustup is using
```

### C1. cargo check (fast type-check, std side)
```bash
SKIP_WASM_BUILD=1 cargo +1.93.0 check -p encointer-kusama-runtime
```
`SKIP_WASM_BUILD=1` skips the wasm build script for fast iteration. Type-check only.

### C2. Real wasm runtime build
```bash
cargo +1.93.0 check -p encointer-kusama-runtime
```
Without `SKIP_WASM_BUILD`, the build script invokes `substrate-wasm-builder` which produces:
```
target/debug/wbuild/encointer-kusama-runtime/encointer_kusama_runtime.wasm
```
This is the canonical wasm verification.

**Anti-pattern**: do NOT run `cargo check -p encointer-kusama-runtime --no-default-features --target=wasm32-unknown-unknown`. It hits an upstream `cumulus-primitives-proof-size-hostfunction` bug (the `#[runtime_interface]` macro emits code referencing `ProofSizeExt`, which is `#[cfg(feature = "std")]`-gated; under direct wasm-target with no_std it fails). Substrate runtimes are not meant to be checked with that command — `substrate-wasm-builder` is.

### C3. runtime-benchmarks feature
```bash
SKIP_PALLET_REVIVE_FIXTURES=1 cargo +1.93.0 check -p encointer-kusama-runtime --features runtime-benchmarks
```
`pallet-revive-fixtures` (a transitive dep pulled in by the benchmark feature) needs `solc` to compile its Solidity test fixtures. Encointer doesn't use pallet-revive, so skipping is sound. Without this env var the build fails with `Failed to execute solc`.

### C4. tests
```bash
cargo +1.93.0 test -p encointer-kusama-runtime
```

## Phase D — verify chain-spec-generator
```bash
cargo +1.93.0 check -p chain-spec-generator --no-default-features --features encointer-kusama
cargo +1.93.0 check -p chain-spec-generator --no-default-features --features all-kusama
```
The second is a sanity check that `encointer-kusama` is included in the `all-kusama` feature rollup.

## Phase E — bump member crate versions

Once Phases A–D are green, bump member versions before publishing.

```bash
cd <encointer-pallets>
.claude/skills/upgrade-polkadot-sdk/scripts/bump-member-versions.sh --strategy minor
cargo check --workspace   # refreshes Cargo.lock with the new versions
```

The default strategy is `minor` (`22.x.y → 22.(x+1).0` across all member crates). The script also bumps the matching `version = "..."` strings in the root `[workspace.dependencies]` block so internal patch deps still resolve.

The encointer versioning policy (from `README.md` "Versioning") is "major version per polkadot-sdk minor version". For unstable RCs we deviated to a minor bump on encointer/pallets#477. Pass `--strategy major` if the operator wants the canonical major bump.

## Hand-off — publish

The operator publishes the bumped crates manually using the dependency-ordered `cargo release publish ...` recipe in `README.md` ("crates.io" section). The skill MUST NOT run publish commands — this is a user-driven step.

After publication, the operator confirms ("the new versions are on crates.io" or similar) and the skill resumes at Phase F.

## Phase F — clean up `<runtimes>`

Goal: convert `<runtimes>` from "patches pointing at local encointer crates" to "encointer crates fetched from crates.io at the new versions".

### F1. Bump `<runtimes>` workspace-dep versions
For every encointer entry in `<runtimes>/Cargo.toml [workspace.dependencies]`, update the `version = "~22.x.0"` to match the version that was just published (the same number Phase E wrote into encointer-pallets). The mapping is the table the bump-member-versions script printed in its summary.

### F2. Delete the `[patch.crates-io]` block
Remove the block we added in B1.

### F3. Refresh `<runtimes>/Cargo.lock`
```bash
cd <runtimes> && cargo +1.93.0 check -p encointer-kusama-runtime
```
Cargo will fetch the newly-published versions from crates.io and update the lockfile. Confirm with:
```bash
grep -A1 'name = "pallet-encointer-balances"' Cargo.lock
```
should show the new version (e.g. `22.3.0`).

## Phase G — closing check: duplicate substrate deps

```bash
cd <runtimes>
cargo +1.93.0 tree --workspace --duplicates 2>&1 \
  | grep -E '^(sp-|frame-|pallet-|cumulus-|polkadot-|xcm|staging-xcm|sc-|substrate-|encointer-)'
```

What to look for:
- **No encointer-introduced duplicate cohort.** Specifically: no `frame-support` v46 next to v47, no `sp-runtime` v46 next to v47, no `sp-core` v40 next to v41, etc. (The exact version numbers here are from the May 2026 cohort; for future runs the boundary is "old SDK release" vs "new SDK release". The check is structural: if encointer is dragging in an older substrate cohort, you'll see a v(N-1) line for every major substrate crate.)
- **No `warning: patch ... was not used in the crate graph`** lines anywhere in the cargo output during Phase B–E. Their appearance is the silent regression mode of the sequencing rule: it means the local crate version no longer matches the `~22.x.y` requirement in /runtimes, so cargo bypassed the patch and pulled the old crate from crates.io.
- **Pre-existing duplicates from subxt and other devtools** (e.g. `sp-api v39`, `sp-runtime v44`, `sc-network v0.53`) are out of scope. They predate this work.

## Variants

**Stable releases.** Phase A uses `cargo psvm`; everything else is the same.

**Subsequent RCs of the same release** (e.g. `unstable2604-rc2` after `rc1`). Re-run Phase A only, with the new RC versions — the structural changes from Phase B onward have already been done in the previous RC pass.

**Rollback** (if the upgrade needs to be abandoned mid-flight). Revert `encointer-pallets/Cargo.toml` `[workspace.dependencies]` to the previous values; revert the `[patch.crates-io]` block + uncomments in `<runtimes>` (`git checkout`). If Phase E ran but publication didn't happen, also revert the member-version bumps.

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `error[E0433]: failed to resolve: use of unresolved module or unlinked crate std` in pallet under `cargo check --workspace --no-default-features` | Feature unification: std-only workspace members force frame-support std on, which makes `std_enabled!`-gated genesis_build emit `std::result::Result` into no_std crates | Use `./scripts/run_for_all_no_std_crates.sh` (per-crate) instead of the workspace check |
| `Cannot compile the WASM runtime: no standard library sources found at .../rustlib/src/rust/` | `substrate-wasm-builder` requires `rust-src` on the active toolchain, not installed | `rustup component add rust-src --toolchain <name>` (both nightly and the active stable) |
| `Error: Failed to execute solc` from `pallet-revive-fixtures` build script | Solidity fixtures need the `solc` binary (only relevant for `--features runtime-benchmarks`) | `SKIP_PALLET_REVIVE_FIXTURES=1` env var |
| `error[E0425]: cannot find type ProofSizeExt` / `cannot find type Vec` in `cumulus-primitives-proof-size-hostfunction` | You ran `cargo check --target=wasm32-unknown-unknown` directly on a substrate runtime — upstream cumulus bug under that exact invocation | Don't use direct wasm-target check. Use `cargo check` (without `--target`) so substrate-wasm-builder runs via build.rs |
| Wasm script seems stuck for many minutes, no output | `find . -name Cargo.toml` is iterating into `target/package/` (stale `cargo package` artifacts) | `rm -rf target/package` |
| `warning: patch ... was not used in the crate graph` | Local crate version doesn't satisfy the `~22.x.y` requirement in /runtimes (you bumped member versions before publication) | Either bump /runtimes requirements to match local versions (only valid post-publication) or revert the member-version bumps |
| Duplicate substrate cohorts in `cargo tree --duplicates` after Phase F | Forgot to update some encointer workspace-dep `version = "~22.x.0"` strings in `<runtimes>/Cargo.toml`, so cargo resolved the old version | Re-check every encointer entry against the Phase E summary table |

## Notes on what NOT to do
- Don't touch member `package.version` during Phases A–D. They must stay at the currently-published values for the patches to resolve.
- Don't bump `<runtimes>` workspace-dep version strings before publication. Committing `version = "~22.3.0"` against an unpublished crate bricks the workspace for anyone without the local patch.
- Don't add a `rust-toolchain.toml` to `<runtimes>` to force the toolchain. The runtimes workspace's own `.github/env` is the source of truth for upstream; we leave it alone.
- Don't run publish commands from this skill. Publish is a user-driven step (see "Hand-off").
