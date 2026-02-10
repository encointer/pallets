//! Core ZK circuit and prover logic for Encointer offline payments.
//!
//! Pure arkworks code — no substrate/FRAME dependencies.
//! Used by the pallet (on-chain, std-only for prover) and the mobile FFI crate.

pub mod circuit;

#[cfg(feature = "std")]
pub mod prover;
