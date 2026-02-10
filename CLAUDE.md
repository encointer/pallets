# Encointer Pallets

Substrate FRAME pallets for the Encointer protocol - a proof-of-personhood system with community currencies.

## Structure

Cargo workspace with pallets in subdirectories:
- `balances/` - Community currency balances (fixed-point `BalanceType` = i64F64)
- `ceremonies/` - Proof-of-personhood ceremony logic
- `communities/` - Community registration and metadata
- `offline-payment/` - ZK-based offline payments (Groth16 on BN254)
- `scheduler/` - Ceremony phase scheduling
- `democracy/`, `faucet/`, `treasuries/`, `vouches/`, etc.

Each pallet follows standard FRAME structure: `src/lib.rs` (pallet definition), `src/tests.rs`, `src/mock.rs`.

## Build & Test

```bash
# Build all pallets (no_std by default)
cargo build --release

# Run all tests
cargo test

# Test specific pallet
cargo test -p pallet-encointer-balances
cargo test -p pallet-encointer-offline-payment

# Check no_std compatibility (what runtime uses)
cargo check --no-default-features
./scripts/run_for_all_no_std_crates.sh check --no-default-features --target=wasm32-unknown-unknown
```

## dependency updates

For polkadot sdk crates, use cargo psvm to globally update to new sdk releases. use stable<YYMM> releases unless I tell you otherwise

## Key Types

- `CommunityIdentifier` - Geohash-based community ID
- `BalanceType` - Fixed-point i64F64 for community currency amounts
- `ReputationCount` - Number of ceremonies attended (proof-of-personhood score)

## offline-payment Pallet

Uses arkworks for ZK proofs (NOT Circom despite what old docs may say):
- `src/circuit.rs` - R1CS circuit using ark-r1cs-std, ark-crypto-primitives Poseidon
- `src/prover.rs` - Groth16 proof generation (std only)
- `src/verifier.rs` - On-chain verification using ark-groth16

The circuit proves knowledge of `zk_secret` matching a registered commitment, and binds payment details (recipient, amount, cid) with a nullifier for double-spend prevention.

## Dependencies

- Polkadot SDK v45+ (frame-support, frame-system, sp-*)
- substrate-fixed for BalanceType
- ark-* crates for ZK (offline-payment only)

## Common Issues

1. **no_std errors**: Ensure `default-features = false` for all runtime deps
2. **BalanceType conversion**: Use `BalanceType::from_num()` not `From<u64>`
3. **Test mock setup**: See `test-utils/` for shared mock runtime helpers
