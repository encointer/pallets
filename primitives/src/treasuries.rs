use crate::{balances::BalanceType, communities::CommunityIdentifier};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};
use sp_core::RuntimeDebug;

#[derive(
	Encode, Decode, Default, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
/// specifies an amount of native tokens which the owner of this option can receive from the
/// community treasury in return for community currency before the expiry date
pub struct SwapNativeOption<NativeBalance, Moment> {
	/// specifies community currency to be swapped for native tokens out of its community
	/// treasury
	pub cid: CommunityIdentifier,
	/// the total amount of native tokens which can be swapped with this option
	pub native_allowance: NativeBalance,
	/// the exchange rate. How many units of community currency will you pay to get one unit of
	/// native tokens? Leave as None if the rate is derived on the spot by either an oracle or an
	/// auction
	pub rate: Option<BalanceType>,
	/// if true, cc will be burned. If false, cc will be put into community treasury
	pub do_burn: bool,
	/// first time of validity for this option
	pub valid_from: Option<Moment>,
	/// the latest time of validity for this option
	pub valid_until: Option<Moment>,
}
