# pallet-encointer-reputation-ring

Bandersnatch key registration and per-community reputation ring publication for the Encointer protocol.

## Overview

After each ceremony cycle, this pallet computes 5 nested reputation rings per community. Each ring groups accounts by
how many of the last 5 ceremonies they attended:

| Ring | Requirement               | Use case                                        |
|------|---------------------------|-------------------------------------------------|
| 1/5  | >= 1 of last 5 ceremonies | Low-friction participation (e.g., deliberation) |
| 2/5  | >= 2 of last 5 ceremonies | Moderate sybil resistance                       |
| 3/5  | >= 3 of last 5 ceremonies | Higher trust (e.g., treasury votes)             |
| 4/5  | >= 4 of last 5 ceremonies | Strong sybil resistance                         |
| 5/5  | All 5 last ceremonies     | Maximum sybil resistance                        |

Rings are strict subsets: 5/5 ⊂ 4/5 ⊂ 3/5 ⊂ 2/5 ⊂ 1/5. Only accounts with a registered Bandersnatch public key are
included.

These rings are general-purpose building blocks for anonymous credential systems. A user proving membership in a ring
proves "I attended at least N of the last 5 ceremonies in community X" without revealing which account they are.
Applications include anonymous deliberation boards, anonymous voting, and anonymous claims.

## How it works

1. **Key registration** — Users call `register_bandersnatch_key` to associate a 32-byte Bandersnatch public key with
   their account.

2. **Ring initiation** — Anyone calls `initiate_rings(community, ceremony_index)` to start computing rings for a past
   ceremony.

3. **Multi-block computation** — Ring computation is split across multiple blocks to stay within weight limits:
    - **Collection phase** (6 steps): Scans the last 5 ceremonies, counting each registered account's attendance.
    - **Building phase** (5 steps): Builds one ring per step, from strictest (5/5) down to loosest (1/5).
    - Each step is driven by calling `continue_ring_computation`.

4. **Result** — `RingMembers` storage contains sorted Bandersnatch public key lists per (community, ceremony_index,
   reputation_level).

## Extrinsics

- `register_bandersnatch_key(key)` — Register or update a Bandersnatch public key.
- `initiate_rings(community, ceremony_index)` — Start ring computation for a past ceremony.
- `continue_ring_computation()` — Advance the computation by one step. Callable by anyone.

## Future work

Ring verifier key (ring-VRF) computation is not yet included. The `bandersnatch-experimental` feature in `sp-core` is
required for that and is not yet stable. Currently only the member lists are published; ring-VRF verifier keys will be
added when the upstream API stabilizes.
