#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::fungibles;
use pallet_transaction_payment::OnChargeTransaction;

pub mod balance_conversion;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use balance_conversion::*;

pub type OnChargeTransactionOf<T> = <T as pallet_transaction_payment::Config>::OnChargeTransaction;

pub type BalanceOf<T> = <OnChargeTransactionOf<T> as OnChargeTransaction<T>>::Balance;

pub type FungiblesOf<T> = <T as pallet_asset_tx_payment::Config>::Fungibles;

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

pub type AssetBalanceOf<T> = <FungiblesOf<T> as fungibles::Inspect<AccountIdOf<T>>>::Balance;

pub type AssetIdOf<T> = <FungiblesOf<T> as fungibles::Inspect<AccountIdOf<T>>>::AssetId;
