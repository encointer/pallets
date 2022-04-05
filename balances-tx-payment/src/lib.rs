// Copyright (c) 2019 Alain Brenzikofer
// This file is part of Encointer
//
// Encointer is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Encointer is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Encointer.  If not, see <http://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::fungibles;
use pallet_asset_tx_payment::HandleCredit;
use pallet_transaction_payment::OnChargeTransaction;

pub mod balance_conversion;
#[cfg(test)]
mod tests;

const LOG: &str = "encointer";

pub use balance_conversion::*;

pub type OnChargeTransactionOf<T> = <T as pallet_transaction_payment::Config>::OnChargeTransaction;

pub type BalanceOf<T> = <OnChargeTransactionOf<T> as OnChargeTransaction<T>>::Balance;

pub type FungiblesOf<T> = <T as pallet_asset_tx_payment::Config>::Fungibles;

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

pub type AssetBalanceOf<T> = <FungiblesOf<T> as fungibles::Inspect<AccountIdOf<T>>>::Balance;

pub type AssetIdOf<T> = <FungiblesOf<T> as fungibles::Inspect<AccountIdOf<T>>>::AssetId;

pub struct BurnCredit;
impl<T> HandleCredit<<T as frame_system::Config>::AccountId, pallet_encointer_balances::Pallet<T>>
	for BurnCredit
where
	T: frame_system::Config + pallet_encointer_balances::Config,
{
	fn handle_credit(
		_credit: fungibles::CreditOf<AccountIdOf<T>, pallet_encointer_balances::Pallet<T>>,
	) {
		// just doing nothing with the credit, will use the default implementation
		// of fungibles an decrease total issuance.
	}
}
