use core::fmt::Debug;
use frame_support::traits::tokens::{PaymentStatus};
use scale_info::TypeInfo;
use sp_runtime::codec::{FullCodec, MaxEncodedLen};
use sp_runtime::{DispatchError};

/// Can be implemented by `PayFromAccount` using a `fungible` impl, but can also be implemented with
/// XCM/Asset and made generic over assets.
///
/// It is similar to the `frame_support::traits::tokens::Pay`, but it offers a variable source
/// account for the payment.
pub trait Payout {
	/// The type by which we measure units of the currency in which we make payments.
	type Balance;
	/// AccountId
	type AccountId;

	/// The type for the kinds of asset that are going to be paid.
	///
	/// The unit type can be used here to indicate there's only one kind of asset to do payments
	/// with. When implementing, it should be clear from the context what that asset is.
	type AssetKind;
	/// An identifier given to an individual payment.
	type Id: FullCodec + MaxEncodedLen + TypeInfo + Clone + Eq + PartialEq + Debug + Copy;
	/// An error which could be returned by the Pay type
	type Error: Debug + Into<DispatchError>;
	/// Make a payment and return an identifier for later evaluation of success in some off-chain
	/// mechanism (likely an event, but possibly not on this chain).
	fn pay(
		from: &Self::AccountId,
		to: &Self::AccountId,
		asset_kind: Self::AssetKind,
		amount: Self::Balance,
	) -> Result<Self::Id, Self::Error>;

	fn is_asset_supported(asset_id: &Self::AssetKind) -> bool;

	/// Check how a payment has proceeded. `id` must have been previously returned by `pay` for
	/// the result of this call to be meaningful. Once this returns anything other than
	/// `InProgress` for some `id` it must return `Unknown` rather than the actual result
	/// value.
	fn check_payment(id: Self::Id) -> PaymentStatus;
	/// Ensure that a call to pay with the given parameters will be successful if done immediately
	/// after this call. Used in benchmarking code.
	// Todo: check if we want to keep these things
	#[cfg(feature = "runtime-benchmarks")]
	fn ensure_successful(
		who: &Self::AccountId,
		asset_kind: Self::AssetKind,
		amount: Self::Balance,
	);
	/// Ensure that a call to `check_payment` with the given parameters will return either `Success`
	/// or `Failure`.
	#[cfg(feature = "runtime-benchmarks")]
	fn ensure_concluded(id: Self::Id);
}
