## 0. WARNING

This is prototype code. Do not use in production!


## 1. Context

Encointer is a community currency system where payments happen face-to-face. Internet outages at the point of sale make the wallet unusable. The goal: let buyer and seller complete a payment using only their phones (no connectivity), with settlement happening automatically when either party regains internet.

**Why ZK over a pre-signed extrinsic:** A standard Substrate extrinsic requires a sequential nonce and a mortal era anchored to a recent block hash — both are chain state that goes stale. A ZK proof with a **nullifier** replaces the nonce system entirely: each payment gets a unique, independently-verifiable nullifier. No ordering constraints, no era expiry, no stale chain state. Either party can submit the proof at any time.

**Remaining trust assumption:** The seller cannot verify the buyer's balance offline. The buyer could generate proofs totaling more than their balance (economic double-spend). Encointer's reputation system (proof-of-personhood via ceremony attendance) bounds this trust — the seller decides whether to accept based on the buyer's verified reputation count.

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

**Stack:** Circom circuit, Poseidon hash, Groth16 proving system, `mopro` Flutter plugin for mobile proof generation, custom Substrate pallet with `ark-groth16` verifier.

---

## 3. Design Overview

### 3.1 Why nullifiers solve the nonce/era problem

| Problem with pre-signed extrinsic | How nullifiers solve it |
|---|---|
| Nonce is sequential — multiple offline payments must be submitted in order | Each payment has an independent nullifier — any submission order works |
| Nonce goes stale if buyer sends an online tx | Nullifiers are independent of chain state |
| Mortal era expires (~hours/days) | Nullifiers never expire |
| Buyer can race to invalidate nonce (double-spend) | Buyer can't produce two proofs with the same nullifier (ZK constraint) |

**What nullifiers DON'T solve:** A buyer can create multiple proofs for different recipients totaling more than their balance. The chain checks balance at settlement time; the second proof would fail. This is the "limited trust" the seller accepts.

### 3.2 Cryptographic identity model

The encointer wallet uses **sr25519** keys (Ristretto/Curve25519). Verifying sr25519 key derivation inside a SNARK is prohibitively expensive (~100K+ constraints for Curve25519 arithmetic). Instead:

- Derive a **ZK secret** from the account seed: `zk_secret = Poseidon(account_seed_bytes, domain_separator)`
- Compute **commitment** = `Poseidon(zk_secret)`
- Register the commitment on-chain, linked to the sr25519 account
- Inside the ZK circuit, prove `commitment == Poseidon(zk_secret)` — only ~240 constraints

This creates a parallel identity system: the sr25519 key is for normal transactions; the Poseidon commitment is for offline payments. They're linked on-chain by the registration extrinsic.

### 3.3 Data flow and sizes

```
Groth16 proof:           ~192 bytes (2 G1 + 1 G2 on BN254)
Public inputs (5 fields): ~160 bytes (5 × 32-byte field elements)
Display metadata:         ~100 bytes (address, amount, reputation, label)
Total QR payload:         ~450 bytes → ~600 chars base64 → QR version 11 (61×61)
```

---

## 4. ZK Circuit

### 4.1 Circuit definition (Circom)

**File: `circuits/offline_payment.circom`**

```circom
pragma circom 2.1.0;

include "node_modules/circomlib/circuits/poseidon.circom";
include "node_modules/circomlib/circuits/bitify.circom";
include "node_modules/circomlib/circuits/comparators.circom";

template OfflinePayment() {
    // --- Public inputs ---
    signal input commitment;       // Poseidon(zk_secret), registered on-chain
    signal input recipient;        // Poseidon(recipient_pubkey)
    signal input amount;           // transfer amount as field element
    signal input cid_hash;         // Poseidon(community_identifier)
    signal input nullifier;        // unique payment identifier

    // --- Private inputs (witness) ---
    signal input zk_secret;        // derived from account seed
    signal input nonce_secret;     // random per-payment

    // --- Constraint 1: prove key ownership ---
    // commitment == Poseidon(zk_secret)
    component key_hash = Poseidon(1);
    key_hash.inputs[0] <== zk_secret;
    commitment === key_hash.out;

    // --- Constraint 2: nullifier correctness ---
    // nullifier == Poseidon(zk_secret, nonce_secret)
    component null_hash = Poseidon(2);
    null_hash.inputs[0] <== zk_secret;
    null_hash.inputs[1] <== nonce_secret;
    nullifier === null_hash.out;

    // --- Constraint 3: amount is positive and bounded ---
    // amount fits in 128 bits (matches Substrate u128 / FixedU128)
    component amount_bits = Num2Bits(128);
    amount_bits.in <== amount;

    // --- Constraint 4: bind all public inputs ---
    // (implicit — they're part of the SNARK verification equation)
}

component main {public [commitment, recipient, amount, cid_hash, nullifier]}
    = OfflinePayment();
```

### 4.2 Constraint count

| Component | Constraints |
|---|---|
| Poseidon(1) — key derivation | ~240 |
| Poseidon(2) — nullifier | ~240 |
| Num2Bits(128) — range check | ~128 |
| **Total** | **~608** |

### 4.3 Trusted setup

- **Phase 1:** Use Hermez Perpetual Powers of Tau `powersOfTau28_hez_final_10.ptau` (pre-computed, 70+ contributors, sufficient for 2^10 = 1024 constraints)
- **Phase 2:** Circuit-specific, run with snarkjs. Minimum 3 independent contributors from the encointer community. Produces `circuit_final.zkey` (proving key) and `verification_key.json`

Artifacts shipped with the app: `circuit_final.zkey` (~few hundred KB for this circuit size), `offline_payment.wasm` (witness generator)

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
        sender, recipient, cid, amount, nullifier
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

Use `ark-groth16` with `sp-crypto-ec-utils` host functions for BN254 pairing. This gives native-speed EC operations rather than WASM interpretation. The verification is 3 pairings — executes in low single-digit milliseconds.

Dependencies (runtime `Cargo.toml`):
```toml
ark-bn254 = { version = "0.5", default-features = false }
ark-groth16 = { version = "0.5", default-features = false }
ark-serialize = { version = "0.5", default-features = false }
sp-crypto-ec-utils = { version = "0.15", default-features = false }
```

### 5.3 Poseidon hash on-chain

The pallet must compute `Poseidon(recipient_pubkey)` and `Poseidon(cid)` to reconstruct public inputs. Use the `light-poseidon` Rust crate (same parameters as circomlib to ensure consistency):

```toml
light-poseidon = { version = "0.3", default-features = false }
```

### 5.4 Nullifier storage growth

`UsedNullifiers` grows monotonically. **Pruning is dangerous** — if a nullifier is pruned, the same proof could be re-submitted, double-charging the sender.

**Safe approach for v1: no pruning.** Offline payments are low-volume (community currencies, face-to-face). Even 10K payments/year = 320KB state — negligible for a parachain.

**Future: epoch-based pruning.** Add an `epoch` public input to the circuit (e.g., `epoch = block_number / epoch_length`). The pallet only accepts proofs from the current or previous epoch. Nullifiers from epochs older than that are safe to prune because the circuit can no longer produce valid proofs for them. This requires a circuit upgrade (new trusted setup) so it's a v2 concern.

### 5.5 Fee model

The `submit_offline_payment` extrinsic is called by whoever gets online first. Fee options:

1. **Submitter pays** (simplest): The submitter pays the transaction fee. If the seller submits, they bear the fee cost. Can use community currency via existing `ChargeAssetTxPayment`.
2. **Fee from transfer amount**: The pallet deducts a small fee from the transferred amount before crediting the recipient.

Recommend option 1 for simplicity — the seller is motivated to submit (they want their money) and can pay the small fee.

---
## 6. Mobile Prover

### 6.1 Stack choice

**Mopro Flutter plugin** (`mopro_flutter_package`) wrapping arkworks Groth16 via Rust FFI.

Workflow:
1. Write circuit in Circom (section 4)
2. Compile: `circom offline_payment.circom --r1cs --wasm --sym`
3. Trusted setup: `snarkjs groth16 setup ... → circuit_final.zkey`
4. Ship `circuit_final.zkey` + `offline_payment.wasm` as Flutter assets
5. At runtime: call `mopro.generateProof(zkeyPath, witnessInputs)` → returns proof bytes + public inputs

**Fallback** if mopro Flutter plugin is too immature: build a custom Rust FFI bridge using `dart:ffi` + `ark-circom` crate. The FFI surface is two functions: `generate_proof(zkey_bytes, witness_json) → (proof_bytes, public_inputs)` and `verify_proof(vk_bytes, proof_bytes, public_inputs) → bool`.

### 6.2 Expected performance

For ~608 constraints on a mid-range Android phone:
- Proof generation: **200ms – 1s** (arkworks native via FFI)
- Witness generation: negligible (2 Poseidon hashes)
- Total user-perceived latency: **< 2s** — acceptable for a payment flow

### 6.3 ZK secret derivation

In the Flutter app, derive `zk_secret` deterministically from the account seed so the user never needs to manage a separate key:

```dart
// Pseudo-code — actual implementation uses light-poseidon Dart binding or precomputed via FFI
final accountSeed = keyringAccount.pair.secretKey.bytes; // 32 bytes
final domainSep = utf8.encode("encointer-offline-ecash");
final zkSecret = poseidonHash([...accountSeed, ...domainSep]);
```

The Poseidon hash must use identical parameters (t=3, BN254 field, circomlib constants) on both the app side and in the circuit. Use `light-poseidon` Rust crate called via FFI for consistency.

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
   e. Call mopro/FFI: `generateProof(zkey, {zk_secret, nonce_secret, commitment, recipient_hash, amount, cid_hash, nullifier})`
   f. Encode proof + public inputs + metadata into `OfflinePaymentQrCode`
   g. Display QR code for seller to scan
3. Store proof locally for self-settlement attempt
4. Decrement cached local balance (display only)

### 7.4 Receive offline payment (seller)

**New file:** `app/lib/page/offline_payment/receive_offline_payment_page.dart`

When QR scanner detects `encointer-offlinepay`:
1. Decode proof, public inputs, and metadata
2. (Optional) Verify proof locally via mopro `verifyProof()` — fast, gives immediate confidence
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

**Modify:** `packages/ew_polkadart/` — regenerate types after the pallet is added to the runtime. The `submit_offline_payment` extrinsic and new storage items will be auto-generated by `polkadart_cli`.

### Summary of changes

| Layer | Action | Component |
|-------|--------|-----------|
| **Chain** | Create | `pallet-offline-payment` (Rust, Substrate) |
| **Chain** | Modify | Runtime — add pallet to construct_runtime!, integrate ark-groth16 |
| **Circuit** | Create | `circuits/offline_payment.circom` |
| **Circuit** | Create | Trusted setup artifacts (`.zkey`, `.wasm`, `verification_key.json`) |
| **App** | Create | `service/offline/offline_identity_service.dart` — registration |
| **App** | Create | `service/offline/settlement_service.dart` — auto-settle |
| **App** | Create | `store/offline_payment/offline_payment_store.dart` — persistence |
| **App** | Create | `page/qr_scan/qr_codes/offline_payment.dart` — QR type |
| **App** | Create | `page/offline_payment/receive_offline_payment_page.dart` — seller UI |
| **App** | Create | `page/offline_payment/offline_payment_list_page.dart` — history |
| **App** | Modify | `page/qr_scan/qr_codes/qr_code_base.dart` — add enum variant |
| **App** | Modify | `page/qr_scan/qr_scan_service.dart` — handle new QR |
| **App** | Modify | `page/assets/transfer/payment_confirmation_page/index.dart` — offline mode |
| **App** | Modify | `store/app.dart` — init new stores |
| **App** | Add dep | `mopro_flutter_package` or custom Rust FFI for Groth16 proving |
| **App** | Add dep | `light-poseidon` via FFI for consistent Poseidon hashing |
| **App** | Add asset | `circuit_final.zkey`, `offline_payment.wasm` |

---

## 8. Verification

### 8.1 Circuit tests

```bash
# Compile circuit
circom circuits/offline_payment.circom --r1cs --wasm --sym -o build/

# Generate witness and verify
snarkjs groth16 fullprove input.json build/offline_payment.wasm circuit_final.zkey \
  proof.json public.json
snarkjs groth16 verify verification_key.json public.json proof.json
```

Test cases:
- Valid proof with correct witness → verifies
- Wrong `zk_secret` → proof generation fails / verification fails
- Mismatched nullifier → verification fails
- Amount = 0 or negative → range check fails

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
- Proof generation via mopro/FFI produces valid proof (verify locally)
- Settlement service submits on connectivity restore
- Duplicate settlement (both parties submit) → one succeeds, other gets `NullifierAlreadyUsed`, both mark `confirmed`

### 8.5 Manual E2E test

Two emulators, both in airplane mode:
1. Seller creates invoice → buyer scans
2. Buyer generates proof → shows QR → seller scans → accepts
3. Enable network on seller's device → auto-settlement → balance updated
4. Enable network on buyer's device → sees `NullifierAlreadyUsed` → marks confirmed

# Discussion:

Why I picked Circom:                                                                                                                              
                                                                                                                                                
  1. Mobile prover availability. Mopro (the only Flutter ZK plugin) wraps ark-circom — it reads Circom's .zkey + .wasm format natively. There is no 
  equivalent Noir/Barretenberg Flutter plugin. Using Noir means compiling Barretenberg (large C++ codebase) for Android/iOS and writing custom    
  platform channel bridges from scratch.
  2. Proof size → QR fit. Groth16 proofs are ~192 bytes (~256 chars base64). Noir's default UltraPlonk backend produces ~868 byte proofs (~1,160
  chars base64). With metadata, the Noir QR payload hits ~1,400 chars — still fits (QR v17, 85×85) but noticeably denser and harder to scan than the
   Groth16 version (QR v11, 61×61).
  3. Substrate verification. Multiple working Groth16 verifier pallets exist (substrate-groth16, zkVerify). No UltraPlonk/Barretenberg verifier
  pallet exists for Substrate — you'd write one from scratch.

  Where Noir wins:

  - No trusted setup. UltraPlonk uses a universal SRS (no per-circuit ceremony). This eliminates the Phase 2 ceremony entirely — a real security and
   operational advantage.
  - Better DX. Rust-like syntax, type system, Nargo package manager vs. Circom's signal-based DSL.
  - Built-in Poseidon in noir-stdlib — no circomlib dependency.
  - Momentum. Aztec-backed, more active development trajectory.

  The honest trade-off: Circom is the pragmatic choice today because of the mobile toolchain gap. Noir is architecturally superior (especially the
  no-trusted-setup property) but requires building the Flutter FFI bridge to Barretenberg and writing a custom Substrate UltraPlonk verifier — both
  significant efforts.

  If the project timeline allows, Noir + custom Barretenberg FFI + custom verifier pallet is the better long-term investment. If you want the
  shortest path to a working prototype, Circom + Mopro + existing Groth16 pallet patterns wins.   


The app needs to work with iOS and android. does that affect your choice circom/noir or other ffi? 

● Yes, it reinforces Circom + Groth16 via arkworks/Mopro. Here's why:                                                                               
   
  Rust FFI (arkworks, what Mopro uses): Rust has first-class cross-compilation targets for both aarch64-apple-ios and aarch64-linux-android.        
  flutter_rust_bridge generates the Dart↔Rust FFI bindings for both platforms from a single Rust codebase. cargo-ndk handles Android NDK
  integration. arkworks is pure Rust with zero system dependencies — it compiles cleanly to both targets. Mopro was specifically built for this (iOS
   + Android mobile proving) and its Flutter plugin targets both.                  

  C++ FFI (Barretenberg, what Noir uses): Barretenberg is a large C++ codebase with CMake, threading, and platform-specific assembly optimizations.
  Cross-compiling C++ for Android (NDK toolchain) and iOS (Xcode) simultaneously is significantly more painful than Rust. No Flutter plugin exists —
   you'd write platform channels in both Kotlin and Swift, each calling the native C++ library through JNI / C bridging respectively. Two separate
  build integrations to maintain.

  Bottom line: The iOS + Android requirement makes the Rust-based stack (arkworks → Mopro → Flutter) the clear winner. The C++ cross-compilation
  overhead for Barretenberg across two mobile platforms is a project in itself.

  I'll keep the plan as Circom + Groth16 + Mopro. If Mopro's Flutter plugin (v0.0.1) turns out too immature, the fallback is a custom dart:ffi
  bridge to arkworks — still Rust, still cross-compiles cleanly to both platforms. Want me to finalize the plan as-is?
