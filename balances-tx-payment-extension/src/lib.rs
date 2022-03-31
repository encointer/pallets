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
// Balance type alias.
pub type BalanceOf<T> = <OnChargeTransactionOf<T> as OnChargeTransaction<T>>::Balance;

pub type AssetBalanceOf<T> =
	<<T as pallet_asset_tx_payment::Config>::Fungibles as fungibles::Inspect<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

pub type AssetIdOf<T> = <<T as pallet_asset_tx_payment::Config>::Fungibles as fungibles::Inspect<
	<T as frame_system::Config>::AccountId,
>>::AssetId;
