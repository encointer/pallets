# pallet-encointer-offline-payment

## 1. Context

Encointer is a community currency system where payments happen face-to-face. Internet outages at the point of sale make
the wallet unusable. The goal: let buyer and seller complete a payment using only their phones (no connectivity), with
settlement happening automatically when either party regains internet.

**Why ZK over a pre-signed extrinsic:** A standard Substrate extrinsic requires a sequential nonce and a mortal era
anchored to a recent block hash — both are chain state that goes stale. A ZK proof with a **nullifier** replaces the
nonce system entirely: each payment gets a unique, independently-verifiable nullifier. No ordering constraints, no era
expiry, no stale chain state. Either party can submit the proof at any time.

**Remaining trust assumption:** The seller cannot verify the buyer's balance offline. The buyer could generate proofs
totaling more than their balance (economic double-spend). Encointer's reputation system (proof-of-personhood via
ceremony attendance) bounds this trust — the seller decides whether to accept based on the buyer's verified reputation
count.

## 2. Executive Summary

```
ONE-TIME SETUP (online):
  Buyer derives zk_secret = Poseidon(account_seed, "ecash")
  Buyer registers commitment = Poseidon(zk_secret) on-chain

AT POS (both offline):
  1. Seller shows invoice QR                    [existing feature]
  2. Buyer scans, confirms amount
  3. Buyer generates Groth16 proof:
     - proves knowledge of zk_secret matching registered commitment
     - commits to (recipient, amount, cid) in public inputs
     - produces unique nullifier = Poseidon(zk_secret, random_nonce)
  4. Buyer displays QR: proof + public inputs    [~500 bytes → QR]
  5. Seller scans, sees: sender, amount, reputation
  6. Seller accepts → stores proof locally

SETTLEMENT (either party online):
  7. Submit proof to chain via pallet extrinsic
  8. Chain verifies Groth16 proof, checks nullifier not spent,
     executes encointer_balances::transfer
  9. Marks nullifier as used
```

**Stack:** Pure Rust arkworks circuit (ark-r1cs-std), Poseidon hash (ark-crypto-primitives), Groth16 proving system (
ark-groth16), custom Substrate pallet with on-chain verification.

---

## 3. Design Overview

### 3.1 Why nullifiers solve the nonce/era problem

| Problem with pre-signed extrinsic                                          | How nullifiers solve it                                                |
|----------------------------------------------------------------------------|------------------------------------------------------------------------|
| Nonce is sequential — multiple offline payments must be submitted in order | Each payment has an independent nullifier — any submission order works |
| Nonce goes stale if buyer sends an online tx                               | Nullifiers are independent of chain state                              |
| Mortal era expires (~hours/days)                                           | Nullifiers never expire                                                |
| Buyer can race to invalidate nonce (double-spend)                          | Buyer can't produce two proofs with the same nullifier (ZK constraint) |

**What nullifiers DON'T solve:** A buyer can create multiple proofs for different recipients totaling more than their
balance. The chain checks balance at settlement time; the second proof would fail. This is the "limited trust" the
seller accepts.

### 3.2 Cryptographic identity model

The encointer wallet uses **sr25519** keys (Ristretto/Curve25519). Verifying sr25519 key derivation inside a SNARK is
prohibitively expensive (~100K+ constraints for Curve25519 arithmetic). Instead:

- Derive a **ZK secret** from the account seed: `zk_secret = Poseidon(account_seed_bytes, domain_separator)`
- Compute **commitment** = `Poseidon(zk_secret)`
- Register the commitment on-chain, linked to the sr25519 account
- Inside the ZK circuit, prove `commitment == Poseidon(zk_secret)` — only ~240 constraints

This creates a parallel identity system: the sr25519 key is for normal transactions; the Poseidon commitment is for
offline payments. They're linked on-chain by the registration extrinsic.

### 3.3 Data flow and sizes

```
Groth16 proof:           ~192 bytes (2 G1 + 1 G2 on BN254)
Public inputs (5 fields): ~160 bytes (5 × 32-byte field elements)
Display metadata:         ~100 bytes (address, amount, reputation, label)
Total QR payload:         ~450 bytes → ~600 chars base64 → QR version 11 (61×61)
```

---

## 4. ZK Circuit

### 4.1 Circuit definition (arkworks R1CS)

**File: `src/circuit.rs`**

```rust
use ark_bn254::Fr;
use ark_crypto_primitives::sponge::poseidon::{constraints::PoseidonSpongeVar, PoseidonConfig};
use ark_r1cs_std::{fields::fp::FpVar, prelude::*};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

/// The offline payment circuit
pub struct OfflinePaymentCircuit {
    pub poseidon_config: PoseidonConfig<Fr>,
    // Public inputs
    pub commitment: Fr,      // Poseidon(zk_secret), registered on-chain
    pub recipient_hash: Fr,  // hash of recipient account
    pub amount: Fr,          // transfer amount as field element
    pub cid_hash: Fr,        // hash of community identifier
    pub nullifier: Fr,       // unique payment identifier
    // Private inputs (witnesses)
    pub zk_secret: Fr,       // derived from account seed
    pub nonce: Fr,           // random per-payment
}

impl ConstraintSynthesizer<Fr> for OfflinePaymentCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        // Allocate public inputs
        let commitment_var = FpVar::new_input(cs.clone(), || Ok(self.commitment))?;
        let nullifier_var = FpVar::new_input(cs.clone(), || Ok(self.nullifier))?;
        // ... recipient_hash, amount, cid_hash also as public inputs

        // Allocate private witnesses
        let zk_secret_var = FpVar::new_witness(cs.clone(), || Ok(self.zk_secret))?;
        let nonce_var = FpVar::new_witness(cs.clone(), || Ok(self.nonce))?;

        // Constraint 1: commitment = Poseidon(zk_secret)
        let mut commitment_sponge = PoseidonSpongeVar::new(cs.clone(), &self.poseidon_config);
        commitment_sponge.absorb(&vec![zk_secret_var.clone()])?;
        let computed_commitment = commitment_sponge.squeeze_field_elements(1)?[0].clone();
        computed_commitment.enforce_equal(&commitment_var)?;

        // Constraint 2: nullifier = Poseidon(zk_secret, nonce)
        let mut nullifier_sponge = PoseidonSpongeVar::new(cs.clone(), &self.poseidon_config);
        nullifier_sponge.absorb(&vec![zk_secret_var, nonce_var])?;
        let computed_nullifier = nullifier_sponge.squeeze_field_elements(1)?[0].clone();
        computed_nullifier.enforce_equal(&nullifier_var)?;

        Ok(())
    }
}
```

### 4.2 Constraint count

The circuit generates constraints for two Poseidon sponge operations on the BN254 scalar field.

### 4.3 Trusted setup

**File: `src/prover.rs`**

The trusted setup uses arkworks' Groth16 circuit-specific setup:

```rust
use ark_groth16::Groth16;
use ark_bn254::Bn254;

pub struct TrustedSetup {
    pub proving_key: ProvingKey<Bn254>,
    pub verifying_key: VerifyingKey<Bn254>,
}

impl TrustedSetup {
    pub fn generate_with_seed(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        let circuit = OfflinePaymentCircuit::new(/* dummy values */);
        let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng)
            .expect("Setup failed");
        Self { proving_key: pk, verifying_key: vk }
    }
}
```

**For production:** Use a proper MPC ceremony rather than a deterministic seed. The test seed (`TEST_SETUP_SEED`) is for
development only.

Artifacts: Serialized `ProvingKey<Bn254>` and `VerifyingKey<Bn254>` (arkworks canonical serialization)

---

## 5. On-Chain Pallet

### 5.1 `pallet-offline-payment`

New Substrate pallet added to the encointer parachain runtime.

**Storage:**

```rust
/// Maps account → Poseidon(zk_secret) commitment.
/// Set once via register_offline_identity().
#[pallet::storage]
pub type OfflineIdentities<T: Config> =
StorageMap<_, Blake2_128Concat, T::AccountId, [u8; 32], OptionQuery>;

/// Set of spent nullifiers. Prevents double-submission of the same proof.
#[pallet::storage]
pub type UsedNullifiers<T: Config> =
StorageMap<_, Blake2_128Concat, [u8; 32], (), OptionQuery>;

/// Groth16 verification key, set by governance.
/// Serialized ark-groth16 VerifyingKey<Bn254>.
#[pallet::storage]
pub type Vk<T: Config> = StorageValue<_, BoundedVec<u8, MaxVkSize>, OptionQuery>;
```

**Extrinsics:**

```rust
/// One-time: link a Poseidon commitment to this account.
/// Called by the account owner while online.
#[pallet::call_index(0)]
pub fn register_offline_identity(
    origin: OriginFor<T>,
    commitment: [u8; 32],
) -> DispatchResult {
    let who = ensure_signed(origin)?;
    ensure!(!OfflineIdentities::<T>::contains_key(&who), Error::<T>::AlreadyRegistered);
    OfflineIdentities::<T>::insert(&who, commitment);
    Self::deposit_event(Event::OfflineIdentityRegistered { who, commitment });
    Ok(())
}

/// Submit an offline payment proof. Callable by ANYONE — submitter ≠ sender.
/// Verifies Groth16 proof, checks nullifier, executes transfer.
#[pallet::call_index(1)]
pub fn submit_offline_payment(
    origin: OriginFor<T>,
    proof: BoundedVec<u8, MaxProofSize>,    // ~192 bytes
    sender: T::AccountId,
    recipient: T::AccountId,
    amount: BalanceType,                     // i64F64 fixed-point
    cid: CommunityIdentifier,
    nullifier: [u8; 32],
) -> DispatchResult {
    ensure_signed(origin)?; // anyone can submit

    // 1. Sender must have registered offline identity
    let commitment = OfflineIdentities::<T>::get(&sender)
        .ok_or(Error::<T>::NoOfflineIdentity)?;

    // 2. Nullifier must be fresh
    ensure!(
        !UsedNullifiers::<T>::contains_key(&nullifier),
        Error::<T>::NullifierAlreadyUsed
    );

    // 3. Reconstruct public inputs
    let recipient_hash = poseidon_hash(&recipient.encode());
    let cid_hash = poseidon_hash(&cid.encode());
    let public_inputs = [
        field_from_bytes(commitment),
        field_from_bytes(recipient_hash),
        amount_to_field(amount),
        field_from_bytes(cid_hash),
        field_from_bytes(nullifier),
    ];

    // 4. Verify Groth16 proof
    let vk_bytes = Vk::<T>::get().ok_or(Error::<T>::NoVerifyingKey)?;
    let vk = deserialize_vk(&vk_bytes);
    ensure!(
        groth16_verify(&vk, &proof, &public_inputs),
        Error::<T>::InvalidProof
    );

    // 5. Execute community currency transfer
    pallet_encointer_balances::Pallet::<T>::transfer(
        sender.clone(), recipient.clone(), cid, amount
    )?;

    // 6. Record nullifier
    UsedNullifiers::<T>::insert(&nullifier, ());

    Self::deposit_event(Event::OfflinePaymentSettled {
        sender,
        recipient,
        cid,
        amount,
        nullifier
    });
    Ok(())
}

/// Governance: set or update the Groth16 verification key.
#[pallet::call_index(2)]
pub fn set_verification_key(
    origin: OriginFor<T>,
    vk: BoundedVec<u8, MaxVkSize>,
) -> DispatchResult {
    ensure_root(origin)?;
    Vk::<T>::put(vk);
    Ok(())
}
```

### 5.2 Groth16 verification on-chain

Use `ark-groth16` with `sp-crypto-ec-utils` host functions for BN254 pairing. This gives native-speed EC operations
rather than WASM interpretation. The verification is 3 pairings — executes in low single-digit milliseconds.

Dependencies (runtime `Cargo.toml`):

```toml
ark-bn254 = { version = "0.5", default-features = false }
ark-groth16 = { version = "0.5", default-features = false }
ark-serialize = { version = "0.5", default-features = false }
sp-crypto-ec-utils = { version = "0.15", default-features = false }
```

### 5.3 Poseidon hash on-chain

The pallet uses `ark-crypto-primitives` for Poseidon hashing, ensuring consistency between the circuit constraints and
on-chain verification:

```rust
use ark_crypto_primitives::sponge::{
    poseidon::{PoseidonConfig, PoseidonSponge},
    CryptographicSponge,
};

pub fn poseidon_hash(config: &PoseidonConfig<Fr>, inputs: &[Fr]) -> Fr {
    let mut sponge = PoseidonSponge::new(config);
    for input in inputs {
        sponge.absorb(input);
    }
    sponge.squeeze_field_elements::<Fr>(1)[0]
}
```

### 5.4 Nullifier storage growth

`UsedNullifiers` grows monotonically. **Pruning is dangerous** — if a nullifier is pruned, the same proof could be
re-submitted, double-charging the sender.

**Safe approach for v1: no pruning.** Offline payments are low-volume (community currencies, face-to-face). Even 10K
payments/year = 320KB state — negligible for a parachain.

**Future: epoch-based pruning.** Add an `epoch` public input to the circuit (e.g.,
`epoch = block_number / epoch_length`). The pallet only accepts proofs from the current or previous epoch. Nullifiers
from epochs older than that are safe to prune because the circuit can no longer produce valid proofs for them. This
requires a circuit upgrade (new trusted setup) so it's a v2 concern.

### 5.5 Fee model

The `submit_offline_payment` extrinsic is called by whoever gets online first. Fee options:

1. **Submitter pays** (simplest): The submitter pays the transaction fee. If the seller submits, they bear the fee cost.
   Can use community currency via existing `ChargeAssetTxPayment`.
2. **Fee from transfer amount**: The pallet deducts a small fee from the transferred amount before crediting the
   recipient.

Recommend option 1 for simplicity — the seller is motivated to submit (they want their money) and can pay the small fee.

---

## 6. Mobile Prover

### 6.1 Stack choice

**Pure arkworks Rust** compiled to mobile via FFI. The prover uses:

- `ark-groth16` for Groth16 proving
- `ark-bn254` for the BN254 curve
- `ark-crypto-primitives` for Poseidon hashing

**File: `src/prover.rs`**

```rust
pub fn generate_proof(
    proving_key: &ProvingKey<Bn254>,
    zk_secret: Fr,
    nonce: Fr,
    recipient_hash: Fr,
    amount: Fr,
    cid_hash: Fr,
) -> Option<(Proof<Bn254>, Vec<Fr>)> {
    let config = poseidon_config();
    let circuit = OfflinePaymentCircuit::new(
        config, zk_secret, nonce, recipient_hash, amount, cid_hash
    );
    let public_inputs = circuit.public_inputs();
    let proof = Groth16::<Bn254>::prove(proving_key, circuit, &mut rng).ok()?;
    Some((proof, public_inputs))
}
```

For mobile integration: build the arkworks crate as a shared library and call via `dart:ffi` / `flutter_rust_bridge`.

### 6.2 Expected performance

For the circuit on a mid-range Android phone:

- Proof generation: **200ms – 1s** (arkworks native via FFI)
- Witness generation: negligible (2 Poseidon hashes)
- Total user-perceived latency: **< 2s** — acceptable for a payment flow

### 6.3 ZK secret derivation

Derive `zk_secret` deterministically from the account seed:

```rust
// Use the same Poseidon config as the circuit
let config = poseidon_config();
let zk_secret = poseidon_hash( & config, & [account_seed_field, domain_separator_field]);
```

The Poseidon parameters must match exactly between the mobile prover and the on-chain verification.

---

## 7. App-Side Implementation

### 7.1 New QR code type: `encointer-offlinepay`

**File:** `app/lib/page/qr_scan/qr_codes/offline_payment.dart`

```
encointer-offlinepay
v1.0
<base64-proof>           // ~256 chars (192 bytes base64)
<sender-ss58>            // ~48 chars
<recipient-ss58>         // ~48 chars
<cid>                    // ~20 chars
<network>                // ~6 chars
<amount>                 // ~20 chars
<nullifier-hex>          // 64 chars
<reputation-count>       // ~2 chars
<label>                  // ~20 chars
```

Total: ~500 chars. Fits comfortably in QR version 11 (61×61), which holds 772 alphanumeric chars.

**Modify:** `app/lib/page/qr_scan/qr_codes/qr_code_base.dart` — add `offlinepay` to `QrCodeContext` enum.

### 7.2 Offline identity registration (one-time setup)

**New file:** `app/lib/service/offline/offline_identity_service.dart`

On first use (or when user enables offline payments):

1. Derive `zk_secret` from account seed via Poseidon FFI
2. Compute `commitment = Poseidon(zk_secret)`
3. Submit `register_offline_identity(commitment)` extrinsic to chain
4. Store `zk_secret` in `FlutterSecureStorage` (alongside existing key material)
5. Mark account as offline-payment-enabled

### 7.3 Offline payment flow (buyer)

**Modify:** `app/lib/page/assets/transfer/payment_confirmation_page/index.dart`

When offline AND offline identity is registered:

1. Show "Pay Offline" button instead of "Transfer"
2. On confirm:
   a. Generate random `nonce_secret` (32 bytes)
   b. Compute `nullifier = Poseidon(zk_secret, nonce_secret)`
   c. Compute `recipient_hash = Poseidon(recipient_pubkey)`
   d. Compute `cid_hash = Poseidon(cid_bytes)`
   e. Call arkworks FFI: `generate_proof(proving_key, zk_secret, nonce, recipient_hash, amount, cid_hash)`
   f. Encode proof + public inputs + metadata into `OfflinePaymentQrCode`
   g. Display QR code for seller to scan
3. Store proof locally for self-settlement attempt
4. Decrement cached local balance (display only)

### 7.4 Receive offline payment (seller)

**New file:** `app/lib/page/offline_payment/receive_offline_payment_page.dart`

When QR scanner detects `encointer-offlinepay`:

1. Decode proof, public inputs, and metadata
2. (Optional) Verify proof locally via arkworks FFI — fast, gives immediate confidence
3. Display: sender identity, amount, community symbol, reputation count
4. Show trust indicator based on reputation (e.g., "Verified in N ceremonies")
5. Accept / Decline buttons
6. On accept: store in `OfflinePaymentStore` for settlement

### 7.5 Settlement service

**New file:** `app/lib/service/offline/settlement_service.dart`

Listens to `ConnectivityStore.isConnectedToNetwork`. On transition to `true`:

1. Load all pending `OfflinePaymentRecord`s
2. For each, construct and submit the `submit_offline_payment` extrinsic:
   ```dart
   final call = api.encointer.encointerKusama.tx.offlinePayment.submitOfflinePayment(
     proof: proofBytes,
     sender: senderPubkey,
     recipient: recipientPubkey,
     amount: amount,
     cid: communityId,
     nullifier: nullifierBytes,
   );
   ```
3. Submit via existing `TxBuilder` + `submitTx` flow (submitter signs for fee payment)
4. On success: mark `confirmed`
5. On `NullifierAlreadyUsed`: mark `confirmed` (other party already settled)
6. On `InsufficientBalance` or `InvalidProof`: mark `failed`, notify user

### 7.6 Offline payment store

**New file:** `app/lib/store/offline_payment/offline_payment_store.dart`

```dart
@JsonSerializable()
class OfflinePaymentRecord {
  final String proofBase64;
  final String senderAddress;
  final String recipientAddress;
  final String cidFmt;
  final double amount;
  final String nullifierHex;
  final DateTime createdAt;
  final OfflinePaymentRole role;      // sender or receiver
  OfflinePaymentStatus status;        // pending, submitted, confirmed, failed
}

enum OfflinePaymentRole { sender, receiver }
enum OfflinePaymentStatus { pending, submitted, confirmed, failed }
```

MobX store persisted to SharedPreferences, same pattern as `EncointerAccountStore`.

### 7.7 Offline payment history page

**New file:** `app/lib/page/offline_payment/offline_payment_list_page.dart`

Lists all offline payments (sent/received) with status. Accessible from assets page.

### 7.8 QR scanner integration

**Modify:** `app/lib/page/qr_scan/qr_scan_service.dart`

Add handler for `QrCodeContext.offlinepay` → navigate to `ReceiveOfflinePaymentPage`.

### 7.9 Polkadart type generation

**Modify:** `packages/ew_polkadart/` — regenerate types after the pallet is added to the runtime. The
`submit_offline_payment` extrinsic and new storage items will be auto-generated by `polkadart_cli`.

### Summary of changes

| Layer       | Action    | Component                                                                  |
|-------------|-----------|----------------------------------------------------------------------------|
| **Chain**   | Create    | `pallet-offline-payment` (Rust, Substrate)                                 |
| **Chain**   | Modify    | Runtime — add pallet to construct_runtime!, integrate ark-groth16          |
| **Circuit** | Create    | `src/circuit.rs` (arkworks R1CS)                                           |
| **Circuit** | Create    | Trusted setup artifacts (serialized `ProvingKey`, `VerifyingKey`)          |
| **App**     | Create    | `service/offline/offline_identity_service.dart` — registration             |
| **App**     | Create    | `service/offline/settlement_service.dart` — auto-settle                    |
| **App**     | Create    | `store/offline_payment/offline_payment_store.dart` — persistence           |
| **App**     | Create    | `page/qr_scan/qr_codes/offline_payment.dart` — QR type                     |
| **App**     | Create    | `page/offline_payment/receive_offline_payment_page.dart` — seller UI       |
| **App**     | Create    | `page/offline_payment/offline_payment_list_page.dart` — history            |
| **App**     | Modify    | `page/qr_scan/qr_codes/qr_code_base.dart` — add enum variant               |
| **App**     | Modify    | `page/qr_scan/qr_scan_service.dart` — handle new QR                        |
| **App**     | Modify    | `page/assets/transfer/payment_confirmation_page/index.dart` — offline mode |
| **App**     | Modify    | `store/app.dart` — init new stores                                         |
| **App**     | Add dep   | Custom Rust FFI for arkworks Groth16 proving (via `flutter_rust_bridge`)   |
| **App**     | Add asset | Serialized arkworks `ProvingKey<Bn254>`                                    |

---

## 8. Verification

### 8.1 Circuit tests

**File: `src/circuit.rs` (tests module)**

```rust
#[test]
fn test_circuit_constraints_satisfied() {
    let config = poseidon_config();
    let circuit = OfflinePaymentCircuit::new(
        config, zk_secret, nonce, recipient_hash, amount, cid_hash
    );
    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit.generate_constraints(cs.clone()).unwrap();
    assert!(cs.is_satisfied().unwrap());
}

#[test]
fn test_circuit_with_wrong_commitment_fails() {
    // Tamper with commitment → constraints not satisfied
}
```

Run with: `cargo test -p pallet-encointer-offline-payment`

Test cases:

- Valid proof with correct witness → verifies
- Wrong `zk_secret` → constraint violation
- Mismatched nullifier → constraint violation
- Tampered commitment → constraint violation

### 8.2 Pallet unit tests

Standard Substrate pallet tests (`#[cfg(test)]`):

- `register_offline_identity` stores commitment correctly
- `submit_offline_payment` with valid proof → transfer succeeds, nullifier stored
- Same nullifier submitted twice → second call returns `NullifierAlreadyUsed`
- Invalid proof bytes → `InvalidProof`
- Unregistered sender → `NoOfflineIdentity`
- Insufficient balance → transfer error propagated
- Nullifier pruning after N blocks

### 8.3 Integration tests (zombienet local node)

- Full round-trip: register identity → generate proof off-chain → submit → verify balance change
- Two proofs from same sender to different recipients → both succeed (independent nullifiers)
- Economic double-spend: two proofs exceeding balance → first succeeds, second fails with insufficient balance
- Proof submitted by a third party (not sender, not recipient) → succeeds

### 8.4 Flutter app tests

- `OfflinePaymentQrCode` round-trip (encode → decode)
- `OfflinePaymentStore` persistence
- Proof generation via arkworks FFI produces valid proof (verify locally)
- Settlement service submits on connectivity restore
- Duplicate settlement (both parties submit) → one succeeds, other gets `NullifierAlreadyUsed`, both mark `confirmed`

### 8.5 Manual E2E test

Two emulators, both in airplane mode:

1. Seller creates invoice → buyer scans
2. Buyer generates proof → shows QR → seller scans → accepts
3. Enable network on seller's device → auto-settlement → balance updated
4. Enable network on buyer's device → sees `NullifierAlreadyUsed` → marks confirmed

---

## 9. Implementation Notes

### Why pure arkworks Rust

The implementation uses arkworks directly (not Circom) for several reasons:

1. **Single language stack.** Both circuit definition and on-chain verification use the same Rust/arkworks codebase. No
   context switching between Circom DSL and Rust, no format translation between `.zkey`/`.wasm` and arkworks types.

2. **Mobile cross-compilation.** arkworks is pure Rust with zero system dependencies — it compiles cleanly to
   `aarch64-apple-ios` and `aarch64-linux-android`. Use `flutter_rust_bridge` or `dart:ffi` to expose the prover to
   Flutter.

3. **Consistent Poseidon.** Using `ark-crypto-primitives::sponge::poseidon` in both the circuit and on-chain code
   guarantees identical hash outputs. No risk of parameter mismatch between circomlib and a separate Rust
   implementation.

4. **Proof size.** Groth16 proofs are ~192 bytes (~256 chars base64), fitting comfortably in a QR code (version 11,
   61×61).

### Trade-offs

- **Trusted setup required.** Groth16 requires a per-circuit trusted setup ceremony. For production, use MPC with
  multiple independent contributors.
- **Circuit changes require new setup.** Any modification to the circuit constraints requires regenerating the
  proving/verifying keys.
