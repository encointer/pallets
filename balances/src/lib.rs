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

mod impl_fungibles;

pub use crate::weights::WeightInfo;
use codec::EncodeLike;
use core::marker::PhantomData;
use encointer_primitives::{
	balances::{BalanceEntry, BalanceType, Demurrage, FeeConversionFactorType},
	communities::CommunityIdentifier,
	fixed::transcendental::exp,
};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{
		tokens::{fungibles, BalanceConversion},
		Get,
	},
};
use frame_system::{self as frame_system, ensure_signed};
use log::{debug, info};
use pallet_asset_tx_payment::HandleCredit;
use pallet_transaction_payment::OnChargeTransaction;
use sp_runtime::{traits::StaticLookup, AccountId32};
use sp_std::convert::TryInto;

// Logger target
const LOG: &str = "encointer";

/// Demurrage rate per block.
/// Assuming 50% demurrage per year and a block time of 5s
/// ```matlab
/// dec2hex(-round(log(0.5)/(3600/5*24*356) * 2^64),32)
/// ```
/// This needs to be negated in the formula!
// FIXME: how to define negative hex literal?
//pub const DemurrageRate: BalanceType = BalanceType::from_bits(0x0000000000000000000001E3F0A8A973_i128);
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_runtime::AccountId32;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// the default demurrage rate applied to community balances
		#[pallet::constant]
		type DefaultDemurrage: Get<Demurrage>;

		type WeightInfo: WeightInfo;

		type CeremonyMaster: EnsureOrigin<Self::Origin>;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Transfer some balance to another account.
		#[pallet::weight((<T as Config>::WeightInfo::transfer(), DispatchClass::Normal, Pays::Yes))]
		pub fn transfer(
			origin: OriginFor<T>,
			dest: <T::Lookup as StaticLookup>::Source,
			community_id: CommunityIdentifier,
			amount: BalanceType,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(dest)?;
			Self::transfer_(community_id, &from, &to, amount)?;

			Self::deposit_event(Event::Transferred(community_id, from, to, amount));
			Ok(().into())
		}

		#[pallet::weight((<T as Config>::WeightInfo::transfer(), DispatchClass::Normal))]
		pub fn set_fee_conversion_factor(
			origin: OriginFor<T>,
			fee_conversion_factor: FeeConversionFactorType,
		) -> DispatchResultWithPostInfo {
			T::CeremonyMaster::ensure_origin(origin)?;
			<FeeConversionFactor<T>>::put(fee_conversion_factor);
			info!(target: LOG, "set fee conversion factor to {}", fee_conversion_factor);
			Self::deposit_event(Event::FeeConversionFactorUpdated(fee_conversion_factor));
			Ok(().into())
		}
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig {
		pub fee_conversion_factor: FeeConversionFactorType,
	}

	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self { fee_conversion_factor: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			<FeeConversionFactor<T>>::put(&self.fee_conversion_factor);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Token transfer success `[community_id, from, to, amount]`
		Transferred(CommunityIdentifier, T::AccountId, T::AccountId, BalanceType),
		/// fee conversion factor updated successfully
		FeeConversionFactorUpdated(FeeConversionFactorType),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// the balance is too low to perform this action
		BalanceTooLow,
		/// the total issuance would overflow
		TotalIssuanceOverflow,
	}

	#[pallet::storage]
	#[pallet::getter(fn total_issuance_entry)]
	pub type TotalIssuance<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		CommunityIdentifier,
		BalanceEntry<T::BlockNumber>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn balance_entry)]
	pub type Balance<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityIdentifier,
		Blake2_128Concat,
		T::AccountId,
		BalanceEntry<T::BlockNumber>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn demurrage_per_block)]
	pub type DemurragePerBlock<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdentifier, Demurrage, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn fee_conversion_factor)]
	pub(super) type FeeConversionFactor<T: Config> =
		StorageValue<_, FeeConversionFactorType, ValueQuery>;
}

impl<T: Config> Pallet<T> {
	pub fn balance(community_id: CommunityIdentifier, who: &T::AccountId) -> BalanceType {
		Self::balance_entry_updated(community_id, who).principal
	}

	/// get balance and apply demurrage. This is not a noop! It changes state.
	fn balance_entry_updated(
		community_id: CommunityIdentifier,
		who: &T::AccountId,
	) -> BalanceEntry<T::BlockNumber> {
		let entry = <Balance<T>>::get(community_id, who);
		Self::apply_demurrage(entry, Self::demurrage(&community_id))
	}

	pub fn total_issuance(community_id: CommunityIdentifier) -> BalanceType {
		Self::total_issuance_entry_updated(community_id).principal
	}

	/// get total_issuance and apply demurrage. This is not a noop! It changes state.
	fn total_issuance_entry_updated(
		community_id: CommunityIdentifier,
	) -> BalanceEntry<T::BlockNumber> {
		let entry = <TotalIssuance<T>>::get(community_id);
		Self::apply_demurrage(entry, Self::demurrage(&community_id))
	}

	/// calculate actual value with demurrage
	/// the formula applied is
	///   balance_now = balance_last_written
	///     * exp(-1* demurrage_rate_per_block * number_of_blocks_since_last_written)
	fn apply_demurrage(
		entry: BalanceEntry<T::BlockNumber>,
		demurrage: BalanceType,
	) -> BalanceEntry<T::BlockNumber> {
		let current_block = frame_system::Pallet::<T>::block_number();
		let elapsed_time_block_number = current_block - entry.last_update;
		let elapsed_time_u32: u32 = elapsed_time_block_number
			.try_into()
			.ok()
			.expect("blockchain will not exceed 2^32 blocks; qed");
		let elapsed_time = BalanceType::from_num(elapsed_time_u32);
		let exponent: BalanceType = -demurrage * elapsed_time;
		let exp_result: BalanceType = exp(exponent).unwrap();
		//.expect("demurrage should never overflow");
		BalanceEntry {
			principal: entry
				.principal
				.checked_mul(exp_result)
				.expect("demurrage should never overflow"),
			last_update: current_block,
		}
	}

	fn transfer_(
		community_id: CommunityIdentifier,
		from: &T::AccountId,
		to: &T::AccountId,
		amount: BalanceType,
	) -> DispatchResult {
		let mut entry_from = Self::balance_entry_updated(community_id, from);
		ensure!(entry_from.principal >= amount, Error::<T>::BalanceTooLow);
		//FIXME: delete account if it falls below existential deposit
		if from != to {
			let mut entry_to = Self::balance_entry_updated(community_id, to);
			entry_from.principal -= amount;
			entry_to.principal += amount;
			<Balance<T>>::insert(community_id, from, entry_from);
			<Balance<T>>::insert(community_id, to, entry_to);
		} else {
			<Balance<T>>::insert(community_id, from, entry_from);
		}
		Ok(())
	}

	pub fn issue(
		community_id: CommunityIdentifier,
		who: &T::AccountId,
		amount: BalanceType,
	) -> DispatchResult {
		let mut entry_who = Self::balance_entry_updated(community_id, who);
		let mut entry_tot = Self::total_issuance_entry_updated(community_id);
		ensure!(
			entry_tot.principal.checked_add(amount).is_some(),
			Error::<T>::TotalIssuanceOverflow,
		);
		entry_who.principal += amount;
		entry_tot.principal += amount;
		<TotalIssuance<T>>::insert(community_id, entry_tot);
		<Balance<T>>::insert(community_id, who, entry_who);
		debug!(target: LOG, "issue {:?} for {:?}", amount, who);
		Ok(())
	}

	pub fn burn(
		community_id: CommunityIdentifier,
		who: &T::AccountId,
		amount: BalanceType,
	) -> DispatchResult {
		let mut entry_who = Self::balance_entry_updated(community_id, who);
		let mut entry_tot = Self::total_issuance_entry_updated(community_id);
		entry_who.principal = if let Some(res) = entry_who.principal.checked_sub(amount) {
			ensure!(res >= 0, Error::<T>::BalanceTooLow);
			res
		} else {
			return Err(Error::<T>::BalanceTooLow.into())
		};
		entry_tot.principal -= amount;
		//FIXME: delete account if it falls below existential deposit

		<TotalIssuance<T>>::insert(community_id, entry_tot);
		<Balance<T>>::insert(community_id, who, entry_who);
		Ok(())
	}

	/// Returns the community-specific demurrage if it is set. Otherwise returns the
	/// the demurrage defined in the genesis config
	fn demurrage(cid: &CommunityIdentifier) -> BalanceType {
		<DemurragePerBlock<T>>::try_get(cid).unwrap_or_else(|_| T::DefaultDemurrage::get())
	}

	pub fn set_demurrage(cid: &CommunityIdentifier, demurrage: Demurrage) {
		<DemurragePerBlock<T>>::insert(cid, &demurrage);
	}

	pub fn purge_balances(cid: CommunityIdentifier) {
		<Balance<T>>::remove_prefix(cid, None);
	}
}

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

pub struct BalanceToCommunityBalance<T>(PhantomData<T>);
impl<T> BalanceConversion<BalanceOf<T>, AssetIdOf<T>, AssetBalanceOf<T>>
	for BalanceToCommunityBalance<T>
where
	T: Config + pallet_asset_tx_payment::Config,
	encointer_primitives::communities::CommunityIdentifier: From<AssetIdOf<T>>,
	AssetBalanceOf<T>: From<u128>,
	<T as pallet_asset_tx_payment::Config>::Fungibles:
		fungibles::InspectMetadata<<T as frame_system::Config>::AccountId>,
	u128: From<BalanceOf<T>>,
{
	type Error = Error<T>;

	fn to_asset_balance(
		balance: BalanceOf<T>,
		asset_id: AssetIdOf<T>,
	) -> Result<AssetBalanceOf<T>, Self::Error> {
		let decimals =
			<<T as pallet_asset_tx_payment::Config>::Fungibles as fungibles::InspectMetadata<
				<T as frame_system::Config>::AccountId,
			>>::decimals(&asset_id.into());

		let fee_conversion_factor = Pallet::<T>::fee_conversion_factor();
		let balance_u128: u128 = balance.into();
		// 5.233 micro ksm correspond to 0.01 units of the community currency assuming a feeConversionFactor of 10_000
		// the KSM balance parameter comes with 12 decimals
		// 5.233 * 10^6 pKSM = 0.01 * 10^decimals LEU
		// 5.233 * 10^6 pKSM = 0.01 * 10^(decimals - 4) * feeConversionFactor LEU
		// 1 pKSM = (0.01 * 10^(decimals - 4) * feeConversionFactor) / (5.233 * 10^6) LEU
		// 1 pKSM = (0.01 * 10^(decimals - 10) * feeConversionFactor) / 5.233 LEU
		return Ok((balance_u128 *
			(((0.01f64 / 5.233f64) *
				10i128.pow((decimals - 10) as u32) as f64 *
				fee_conversion_factor as f64) as u128))
			.into())
	}
}

pub struct BurnCredit();
impl<T> HandleCredit<<T as frame_system::Config>::AccountId, pallet::Pallet<T>> for BurnCredit
where
	T: Config + frame_system::Config,
{
	fn handle_credit(
		credit: fungibles::CreditOf<<T as frame_system::Config>::AccountId, pallet::Pallet<T>>,
	) {
		// CreditOf means that total supply is larger than sum of all accounts
		// this is because from the AccountId the fee was deducted

		// burn it
		let current_block = frame_system::Pallet::<T>::block_number();
		let new_total_issuance = BalanceEntry {
			principal: <TotalIssuance<T>>::get(credit.asset()).principal -
				Pallet::<T>::fungible_balance_to_balance_type(
					credit.asset().into(),
					credit.peek().into(),
				),
			last_update: current_block,
		};

		<TotalIssuance<T>>::insert(credit.asset(), new_total_issuance)
	}
}

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
