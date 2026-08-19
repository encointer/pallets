---
name: upgrade-polkadot-sdk
description: Use this skill when upgrading encointer-pallets to a new polkadot-sdk release (stable or unstable RC) and re-enabling encointer in the polkadot-fellows runtimes workspace. Triggers include phrases like "upgrade encointer to polkadot-sdk", "bump SDK in encointer pallets", "follow the runtimes SDK upgrade", or any reference to a `polkadot-stableYYMM` / `polkadot-unstableYYMM-rcN` / `polkadot-stableYYMM-N` tag in encointer context. Captures sequencing rules and footguns from the encointer/pallets#477 run (May 2026) and the stable2606-1 run (August 2026).
version: 0.2.0
---

# Upgrading encointer-pallets to a new Polkadot SDK release

## Purpose
This runbook upgrades `encointer-pallets` to a new `polkadot-sdk` release and re-enables encointer in the polkadot-fellows runtimes workspace.

The runtimes repo normally moves first to a new SDK and encointer-pallets follows — though the target may live on an unmerged runtimes PR, or runtimes may not have moved at all (see "Where the runtimes target lives"). While encointer is behind, the runtimes workspace keeps every encointer wiring point commented out behind `TODO ... encointer` markers whose exact wording changes from release to release. Re-enabling them is mechanical but spread across seven files, and the markers alone do not cover every site.

## Path conventions
This skill uses two placeholders for repo paths:
- `<encointer-pallets>` — the encointer-pallets workspace root (where this skill lives, at `<encointer-pallets>/.claude/skills/upgrade-polkadot-sdk/`).
- `<runtimes>` — the polkadot-fellows runtimes workspace root, **assumed to be a sibling of `<encointer-pallets>`** (i.e. `<parent>/encointer-pallets` and `<parent>/runtimes` for some common parent directory).

Resolution at runtime:
- If you're already in the encointer-pallets repo, `<encointer-pallets>` is `pwd` (or whatever `git rev-parse --show-toplevel` reports). `<runtimes>` is `../runtimes` from there.
- The bundled scripts resolve both automatically: `bump-member-versions.sh` and `derive-sdk-versions-from-index.sh` walk up from their own location to find `<encointer-pallets>`, and `derive-sdk-versions.sh` defaults its `<runtimes>` argument to the sibling. Only `derive-sdk-versions.sh` needs `<runtimes>` at all.
- If the layout differs (not siblings, or non-standard names), pass an explicit path to `derive-sdk-versions.sh` and adjust the relative `path = "../encointer-pallets"` strings in the `[patch.crates-io]` block in Phase B accordingly.

## Inputs
Before starting, confirm:
- Target SDK tag, e.g. `polkadot-stable2609` (stable), `polkadot-unstable2609-rc1` (RC) or `polkadot-stable2609-1` (patch release).
- **Which crates.io cohort that tag actually corresponds to** — see "Resolving the target cohort" below. Do this first; it can change what you're targeting.
- Where the runtimes workspace stands. It is *usually* ahead, but not always — see "Where the runtimes target lives".
- The `<encointer-pallets>` and `<runtimes>` paths resolve as described under "Path conventions" above. If the sibling assumption doesn't hold, override `<runtimes>` explicitly.
- Whether the operator wants a member-version bump as part of this work (Phase E). Default: yes.

## Resolving the target cohort (do this before anything else)

**The version numbers in the polkadot-sdk git tree are not the published ones.** The release tooling rewrites them at publish time. At `polkadot-stable2606-1`, `substrate/primitives/arithmetic/Cargo.toml` reads `23.0.0` while the crate on crates.io is `28.0.1` and stays there. Never derive a version table by reading the SDK checkout at a tag.

The authority is the crates.io index, anchored on the `polkadot-sdk` umbrella crate — it depends on ~390 SDK crates with exact requirements, so every version comes off a dependency edge with no guessing:

```bash
.claude/skills/upgrade-polkadot-sdk/scripts/derive-sdk-versions-from-index.sh 2606.0.0
```

Umbrella versions are `YYMM.P.0`. Called with no argument it uses the newest cohort; called with one that doesn't exist it lists what is published and exits non-zero.

**A `-N` patch release usually has no cohort of its own.** SDK patch releases republish only the crates they changed, and sometimes publish nothing at all. `polkadot-stable2606-1` (tagged 2026-08-06) published nothing: there is no `polkadot-sdk 2606.1.0`, and the whole 2606 cohort dates from 2026-07-02. When the umbrella version for a patch release is missing, confirm the patch is irrelevant to us and target the base cohort:

```bash
git -C <polkadot-sdk> diff --stat polkadot-stable2606..polkadot-stable2606-1
```

For 2606-1 that touched only `sc-consensus-grandpa`, `sc-client-db`, `sp-database`, `pallet-utility` and `pallet-multi-asset-bounties` — none of which encointer depends on, so "upgrade to 2606-1" reduced to "upgrade to the 2606 cohort".

**Beware which patch cohort the previous upgrade landed on.** Encointer's pre-2606 pins matched `polkadot-sdk 2604.1.0` (2026-05-21), not `2604.0.0` (2026-05-03) — `frame-support` is identical in both while `frame-benchmarking`, `pallet-balances` and the whole rpc side differ. Anchoring on a single crate rather than the umbrella silently produces the wrong table here.

### The encointer major version
Policy (README "Versioning") is "PSDK minor version as the major version". The polkadot minor is read off the `polkadot-vX.Y.Z` tag that aliases the release tag — they point at the same commit:

```bash
git -C <polkadot-sdk> rev-parse polkadot-stable2606-1^{commit} polkadot-v1.24.1^{commit}  # identical
```

So `stable2606-1` → `polkadot-v1.24.1` → encointer major **24**. Note this is not "current major + 1": members went `22.x → 24.0.0`, skipping 23, because the intervening unstable2604 work took minor bumps instead. `--strategy major` would have produced the wrong number; use `--set` (Phase E).

## Where the runtimes target lives

The skill's original premise was that `<runtimes>`/main is already on the new SDK and encointer follows. That does not always hold. For 2606-1 the fellows' work sat in an **unmerged PR** (polkadot-fellows/runtimes#1223, "Integrate SDK stable2606-1") while main was still on the 2604 cohort, so `derive-sdk-versions.sh` — which reads `<runtimes>/Cargo.toml` — would have returned the *old* versions and looked perfectly plausible doing it.

Fetch the PR branch read-only (works without any GitHub auth):

```bash
git -C <runtimes> fetch --no-tags https://github.com/polkadot-fellows/runtimes.git pull/1223/head:pr-1223
git -C <runtimes> checkout -b <work-branch> pr-1223
```

Sanity-check that the branch really is on the target cohort before trusting it (`grep -E '^(frame-support|sp-runtime) ' Cargo.toml`). Prefer `derive-sdk-versions-from-index.sh` regardless — it needs no runtimes reference at all.

## The sequencing rule (read this first)
**Patches first. Member-version bumps last. `<runtimes>` deps + `[patch.crates-io]` removal happens only after publication.**

Reversing the order silently fails: `<runtimes>/Cargo.toml` already declares tilde requirements like `pallet-encointer-balances = "~22.2.0"`. If encointer member crates are bumped to `22.3.0` before being published, the patches in `<runtimes>` no longer match those tildes and cargo silently falls back to the *old* (still-published) crates, producing duplicate substrate cohorts in the dep graph that don't show up in `cargo check`. The closing check (Phase G) is `cargo tree --duplicates` precisely to catch this.

The phases below enforce the sequence.

### The one safe exception: bumping to an unpublished major

When the member bump crosses into a **major that does not exist on crates.io at all** (e.g. `22.x → 24.0.0`), the danger the rule guards against evaporates. Silent fallback requires an older published version that still satisfies the requirement; with `~24.0.0` against a registry whose newest is `22.9.0`, cargo has nothing to fall back to and fails loudly instead. In that case run Phase E **before** Phase B, set the `<runtimes>` requirements to `~24.0.0` together with the patch block, and you save an entire runtimes rebuild.

Confirm the major really is unpublished before relying on this (`curl -s https://index.crates.io/pa/ll/pallet-encointer-balances | tail -1`). If the target major *is* published — a re-run, a rollback, a second RC — the original ordering applies and there is no shortcut.

Either way, verify with the sweep at the end of Phase B: any `patch ... was not used in the crate graph` warning means the requirement and the local version disagree.

## Phase A — bump encointer-pallets `[workspace.dependencies]`

Working dir: `<encointer-pallets>`.

Goal: update every Polkadot SDK / Substrate / Cumulus / xcm dep in the root `Cargo.toml` `[workspace.dependencies]` table to the target cohort established above. **Do not touch any member crate `package.version` here — that's Phase E, unless the unpublished-major exception applies.**

**Preferred, for every release type** (stable, RC, patch) — read the cohort straight off the crates.io index:
```bash
.claude/skills/upgrade-polkadot-sdk/scripts/derive-sdk-versions-from-index.sh <umbrella-version>
```
It prints current vs target for every SDK crate in our `[workspace.dependencies]`, with no heuristics and no dependency on psvm or on runtimes having moved. See "Resolving the target cohort" above for how to pick `<umbrella-version>`.

Fallbacks, both of which have failure modes worth knowing:

- `cargo psvm -v <stableYYMM>` (needs `cargo install cargo-psvm`) — stable releases only, and it has no notion of `-N` patch cohorts.
- `.claude/skills/upgrade-polkadot-sdk/scripts/derive-sdk-versions.sh <runtimes>` — reads `<runtimes>/Cargo.toml`. Only valid when runtimes is genuinely ahead of us; if it is behind or on the wrong branch it returns stale versions without any error. It also propagates pins that both repos share but that are stale in both, and it cannot resolve the RPC-side crates (`sc-rpc`, `sc-rpc-api`, `sp-blockchain`, `sp-rpc`, `pallet-transaction-payment-rpc`, `sp-keystore`/`sp-inherents`/`sp-keyring`) that the runtimes manifest doesn't pin, leaving a `+1 minor` guess for those.

Whichever source you use, do not hand-correct a version against the SDK checkout — see the warning about rewritten in-tree versions above. `sp-arithmetic` is the standing trap: it has sat at `28.0.1` across 2603, 2604 and 2606 while reading `23.0.0` in-tree, so "it looks out of step with the cohort" is not a reason to touch it.

After applying:
```bash
rm -f Cargo.lock                         # only if you want a totally fresh resolve
cargo check --workspace                  # regenerates Cargo.lock against new versions
grep -A1 'name = "sp-runtime"' Cargo.lock # sanity: should show the new sp-runtime version
```

### Toolchain alignment
Two files carry a toolchain version in this repo and they drift apart — on the 2606 run `rust-toolchain.toml` said `1.93.0` while `.github/env` still said `1.88.0` (only CI's nightly-rustfmt and zepter jobs read the latter, so nothing failed loudly). Keep both at least as new as the SDK's own toolchain.

The authority is the ci-unified image name in the SDK's `.github/env` at the target tag, which encodes stable and nightly:
```bash
git -C <polkadot-sdk> show polkadot-stable2606-1:.github/env
# IMAGE="docker.io/paritytech/ci-unified:bullseye-1.93.0-2026-01-27-v202605151311"
#                                                ^stable  ^nightly
```
`<runtimes>/.github/env` carries its own `RUST_STABLE_VERSION`, which can lag the SDK's (it was `1.91` against the SDK's `1.93.0`); take the higher of the two. If the toolchain moves, `cargo clean` first — a cohort switch invalidates the whole `target/` anyway, and these trees run to hundreds of GB.

### Phase A verification
Run, in order:
```bash
cargo check --workspace
cargo test --all --features runtime-benchmarks --features try-runtime   # what CI gates on
cargo clippy --all-features --all-targets -- -D warnings
./scripts/run_for_all_no_std_crates.sh check --no-default-features --target=wasm32-unknown-unknown
cargo check --manifest-path offline-payment/ffi/Cargo.toml   # standalone workspace, not covered by --workspace
cargo +nightly-<date> fmt --all -- --check
taplo fmt --check
```

**Do not pipe these through `tail` in a wrapper script.** The pipeline exit status is `tail`'s, so `cmd | tail || fail=1` reports success no matter what cargo did — this produced a bogus all-green summary on the 2606 run. Use `set -o pipefail`, or redirect to a log and test `$?` directly.

Data point: the 2604 → 2606 jump required **zero source changes**, in the pallets and in the encointer-kusama runtime alike. A cohort bump landing green on the first `cargo check` is plausible, not suspicious.

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

### B2. Uncomment every encointer marker

**The marker text changes between runs — do not grep for a literal.** May 2026 used `TODO @ggwpez encointer-...`; August 2026 used `TODO(encointer stable2606): ...`. Grep for the shape, not the wording:

```bash
rg -i 'TODO.*encointer' <runtimes>
```

Comment shapes seen so far:
- single-line: `# TODO(...): <toml line>` or `// TODO(...): <rust line>`
- multi-line line-comments: a marker line followed by `// `-prefixed continuation lines (the match arm in `common.rs`, the wrapped type alias in `system_parachains_specs.rs`)
- block: `/* TODO(...) ... */` around a whole item

On the August 2026 run this matched 38 sites across 7 files. On the May 2026 run, 30 sites across the same 7 files (line numbers drift; rely on the grep):
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

### B4. Sweep — and do NOT trust a zero-hit marker sweep
```bash
rg -i 'TODO.*encointer' <runtimes>
```
Must return zero hits. But **zero hits does not mean encointer is fully wired back in.** Some call sites are *deleted outright* rather than commented, leaving nothing to find. On the August 2026 run, `EncointerKusamaChainSpec` had been removed from the `use crate::system_parachains_specs::{...}` list in `chain-spec-generator/src/common.rs` with no marker at all; the sweep was clean and `--features encointer-kusama` still failed with `error[E0433]: failed to resolve: use of undeclared type EncointerKusamaChainSpec`. Import lists are the likely place for this, because rustfmt reflows the list and erases any trace that a name was dropped.

The reliable check is to diff against the last commit where encointer *was* enabled (usually `origin/main`, or the commit before the disabling one) and look at what disappeared:

```bash
cd <runtimes>
git diff origin/main..HEAD | grep '^-' | grep -i encointer | grep -v '^---'
```
Every surviving line should be one you intended to change (the `version = "~XX.Y.Z"` requirements). Anything else is a site you still have to restore, and restoring it to match the old revision exactly is safest:
```bash
git show origin/main:chain-spec-generator/src/common.rs | sed -n '18,27p'
```

Then confirm resolution:
```bash
cd <runtimes> && cargo metadata --format-version 1 > /dev/null
```
Should exit 0 with no `patch ... was not used` warning — this also proves the patch paths and the version requirements agree.

**Transitive encointer crates need no patch entries of their own.** `ep-core`, `encointer-ceremonies-assignment`, `encointer-meetup-validation` and `encointer-offline-payment-core` are reached through `path` dependencies inside the patched crates, and cargo follows those to the local tree. `cargo metadata` prints them as `Updating ... -> vXX.Y.Z` alongside the explicitly patched ones; if it does, they are wired correctly.

## Phase C — verify the encointer-kusama runtime

### C0. Toolchain prep (one-time per machine)
`substrate-wasm-builder` (the build script that compiles the runtime to wasm) needs `rust-src` on both nightly and the active stable toolchain, and the emulated integration tests need the `wasm32v1-none` target:
```bash
rustup component add rust-src --toolchain nightly
rustup component add rust-src --toolchain 1.93.0     # or whatever stable rustup is using
rustup target add wasm32v1-none --toolchain 1.93.0
```
Pin the toolchain explicitly (`cargo +1.93.0 ...`) for everything in `<runtimes>`: it has no `rust-toolchain.toml`, so bare `cargo` picks up whatever rustup's default is — a recent nightly on most machines — and the emulated tests then fail to link with `undefined symbol: ext_*`.

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
.claude/skills/upgrade-polkadot-sdk/scripts/bump-member-versions.sh --set 24.0.0   # stable release
cargo check --workspace   # refreshes Cargo.lock with the new versions
```

The script also bumps the matching `version = "..."` strings in the root `[workspace.dependencies]` block so internal path deps still resolve, and it covers the standalone `offline-payment/ffi` workspace.

Choosing the number:
- **Stable / patch release** → `--set <polkadot-minor>.0.0`, from the `polkadot-vX.Y.Z` alias tag (see "The encointer major version"). Do **not** use `--strategy major`: it computes current-major + 1, which was `23.0.0` when the policy called for `24.0.0`.
- **Unstable RC** → `--strategy minor`, as on encointer/pallets#477.

Add `--dry-run` first to eyeball the table. The script refuses to run if members disagree on their major, and `--set` refuses to move backwards.

Afterwards, confirm nothing was left behind on the old major:
```bash
grep -rn '"2[0-3]\.' --include='*.toml' . | grep -v ./target
```

## Hand-off — publish

The operator publishes the bumped crates manually using the dependency-ordered `cargo release publish ...` recipe in `README.md` ("crates.io" section). The skill MUST NOT run publish commands — this is a user-driven step.

**A dry run cannot validate most of those batches, and its failure is not a real error.** `cargo release publish` without `--execute` packages each crate but aborts the upload (`warning: aborting upload due to dry run`). Several of the README batches contain crates that depend on *siblings in the same batch* — batch 4 has `pallet-encointer-ceremonies` needing `encointer-ceremonies-assignment`, batch 5 has `pallet-encointer-offline-payment` needing `encointer-offline-payment-core`, batch 6's rpc crates need their own `*-rpc-runtime-api`. The dependency was never really uploaded, so packaging the dependent fails with:

```
failed to select a version for the requirement `encointer-ceremonies-assignment = "^24.0.0"`
candidate versions found which didn't match: 22.3.0, 22.2.0, ...
```

That is expected in dry-run mode and says nothing about the release. Only `--execute` gets through, because cargo-release then really uploads each crate and orders the batch by dependency before publishing (it published `assignment` and `meetup-validation` ahead of `ceremonies` even though the command listed `communities` first). If a dry run of the early batches succeeds, that only means their deps were already published by a previous batch.

To check publication state at any point, query the index rather than guessing:
```bash
curl -s https://index.crates.io/pa/ll/pallet-encointer-ceremonies | tail -1 | python3 -c 'import sys,json; print(json.load(sys.stdin)["vers"])'
```

After publication, the operator confirms ("the new versions are on crates.io" or similar) and the skill resumes at Phase F.

## Phase F — clean up `<runtimes>`

Goal: convert `<runtimes>` from "patches pointing at local encointer crates" to "encointer crates fetched from crates.io at the new versions".

### F1. Bump `<runtimes>` workspace-dep versions
For every encointer entry in `<runtimes>/Cargo.toml [workspace.dependencies]`, update the tilde requirement to match the version that was just published (the same number Phase E wrote into encointer-pallets). The mapping is the table the bump-member-versions script printed in its summary.

Skip this if you took the unpublished-major exception — the requirements were already set to the new version back in Phase B, and publication has now made them resolvable.

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
- **`frame-support` and `frame-system` must not appear at all.** They are the sharpest signal: every runtime pallet depends on them, so a second cohort anywhere in the graph shows up here first. On the August 2026 run both were absent from the duplicates list entirely — a single v48.0.0 workspace-wide — which is what "clean" looks like.
- **No encointer-introduced duplicate cohort.** Structurally: if encointer is dragging in the previous SDK release you will see a v(N-1) line for every major substrate crate — e.g. `sp-runtime` v47 beside v48, `sp-core` v41 beside v43 when moving 2604 → 2606.
- **No `warning: patch ... was not used in the crate graph`** lines anywhere in the cargo output during Phase B–E. Their appearance is the silent regression mode of the sequencing rule: it means the local crate version no longer matches the tilde requirement in /runtimes, so cargo bypassed the patch and pulled the old crate from crates.io.
- **Pre-existing duplicates from subxt and other devtools are out of scope.** They predate this work and are far older than the cohort boundary — in August 2026 that was `sp-runtime` v44.0.0, `sp-core` v38.1.0, `sp-api` v39.0.0 and `sp-keystore` v0.44.1, all pulled in by `subxt` 0.43/0.44. Tell them apart from a real regression by their distance from the current cohort: a genuine encointer-introduced duplicate is exactly one release behind, not five.

## Variants

**Stable releases.** Phase E takes `--set <polkadot-minor>.0.0` rather than `--strategy minor`; everything else is the same.

**Patch releases** (`polkadot-stableYYMM-N`). First establish whether a cohort was published for it at all — usually not. If not, and the tag-to-tag diff shows nothing encointer depends on changed, the work is exactly the base release's work. If the base cohort is already what we're on, there is nothing to do beyond confirming it.

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
| `error[E0433]: failed to resolve: use of undeclared type EncointerKusamaChainSpec` while the marker sweep is clean | A call site was deleted outright instead of commented, so there is no marker to find (import lists especially) | Diff against the last encointer-enabled revision — see B4 |
| Version table looks right but a crate is off by a major | Read the version out of the polkadot-sdk checkout at the tag; in-tree numbers are rewritten at publish time | Use `derive-sdk-versions-from-index.sh`; never read versions from the SDK tree |
| Version table matches `<runtimes>` but the build pulls two cohorts | `<runtimes>` was behind, or on a branch that had not moved to the target release | Anchor on the crates.io umbrella crate instead of `<runtimes>` |
| `derive-sdk-versions-from-index.sh` says the umbrella version is not on crates.io | An SDK `-N` patch release that republished nothing | Target the base cohort after confirming with `git diff <base-tag>..<patch-tag>` that no encointer dependency moved |
| `failed to select a version for the requirement ... = "^24.0.0"` during `cargo release publish` | Running without `--execute`: the sibling it needs was "published" by an aborted dry-run upload | Expected in dry-run mode; only `--execute` can get through these batches |
| Verification script reports everything green but a step clearly failed | `cmd \| tail` makes the pipeline exit status `tail`'s | `set -o pipefail`, or log to a file and check `$?` |

## Notes on what NOT to do
- Don't read SDK crate versions out of the polkadot-sdk checkout at a release tag. They are rewritten at publish time and will be wrong.
- Don't treat a zero-hit marker sweep in `<runtimes>` as proof that encointer is fully re-enabled.
- Don't assume a `polkadot-stableYYMM-N` tag has its own crates.io cohort.
- Don't touch member `package.version` during Phases A–D — unless the target major is unpublished, in which case see "The one safe exception". They must otherwise stay at the currently-published values for the patches to resolve.
- Don't bump `<runtimes>` workspace-dep version strings before publication. Committing `version = "~22.3.0"` against an unpublished crate bricks the workspace for anyone without the local patch.
- Don't add a `rust-toolchain.toml` to `<runtimes>` to force the toolchain. The runtimes workspace's own `.github/env` is the source of truth for upstream; we leave it alone.
- Don't run publish commands from this skill. Publish is a user-driven step (see "Hand-off").
