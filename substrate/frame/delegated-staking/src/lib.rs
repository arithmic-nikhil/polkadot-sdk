// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(rustdoc::broken_intra_doc_links)]

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use frame_support::{pallet_prelude::*, traits::{
	fungible::{hold::Mutate as FunHoldMutate, Inspect as FunInspect, Mutate as FunMutate},
	tokens::{Fortitude, Precision, Preservation},
	DefensiveOption,
}, transactional};
use pallet::*;
use sp_runtime::{traits::Zero, DispatchResult, RuntimeDebug, Saturating};
use sp_staking::{
	delegation::{Delegatee, Delegator, StakeBalanceType, StakingDelegationSupport},
	StakerStatus, StakingInterface,
};
use sp_std::{convert::TryInto, prelude::*};

pub type BalanceOf<T> =
	<<T as Config>::Currency as FunInspect<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use sp_staking::delegation::StakingDelegationSupport;

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: FunHoldMutate<Self::AccountId, Reason = Self::RuntimeHoldReason>
			+ FunMutate<Self::AccountId>;
		/// Overarching hold reason.
		type RuntimeHoldReason: From<HoldReason>;

		/// Core staking implementation.
		type CoreStaking: StakingInterface<Balance = BalanceOf<Self>, AccountId = Self::AccountId>
			+ sp_staking::StakingHoldProvider<Balance = BalanceOf<Self>, AccountId = Self::AccountId>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The account cannot perform this operation.
		NotAllowed,
		/// An existing staker cannot become a delegatee.
		AlreadyStaker,
		/// Reward Destination cannot be delegatee account.
		InvalidRewardDestination,
		/// Delegation conditions are not met.
		///
		/// Possible issues are
		/// 1) Account does not accept or has blocked delegation.
		/// 2) Cannot delegate to self,
		/// 3) Cannot delegate to multiple Delegatees,
		InvalidDelegation,
		/// The account does not have enough funds to perform the operation.
		NotEnoughFunds,
		/// Not an existing delegatee account.
		NotDelegatee,
		/// Not a Delegator account.
		NotDelegator,
		/// Some corruption in internal state.
		BadState,
		/// Unapplied pending slash restricts operation on delegatee.
		UnappliedSlash,
		/// Failed to withdraw amount from Core Staking Ledger.
		WithdrawFailed,
	}

	/// A reason for placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason {
		/// Funds held for stake delegation to another account.
		#[codec(index = 0)]
		Delegating,
	}

	// #[pallet::genesis_config]
	// #[derive(frame_support::DefaultNoBound)]
	// pub struct GenesisConfig<T: Config> {}
	//
	// #[pallet::genesis_build]
	// impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
	// 	fn build(&self) {}
	// }

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Delegated { delegatee: T::AccountId, delegator: T::AccountId, amount: BalanceOf<T> },
		Withdrawn { delegatee: T::AccountId, delegator: T::AccountId, amount: BalanceOf<T> },
	}

	/// Map of Delegators to their delegation.
	///
	/// Note: We are not using a double map with delegator and delegatee account as keys since we
	/// want to restrict delegators to delegate only to one account.
	#[pallet::storage]
	pub(crate) type Delegators<T: Config> =
		CountedStorageMap<_, Twox64Concat, T::AccountId, (T::AccountId, BalanceOf<T>), OptionQuery>;

	/// Map of Delegatee to their Ledger.
	#[pallet::storage]
	pub(crate) type Delegatees<T: Config> =
		CountedStorageMap<_, Twox64Concat, T::AccountId, DelegationRegister<T>, OptionQuery>;
}

/// Register of all delegations to a `Delegatee`.
///
/// This keeps track of the active balance of the delegatee that is made up from the funds that are
/// currently delegated to this delegatee. It also tracks the pending slashes yet to be applied
/// among other things.
#[derive(Default, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct DelegationRegister<T: Config> {
	/// Where the reward should be paid out.
	pub payee: T::AccountId,
	/// Sum of all delegated funds to this delegatee.
	#[codec(compact)]
	pub total_delegated: BalanceOf<T>,
	/// Amount that is bonded and held.
	#[codec(compact)]
	pub hold: BalanceOf<T>,
	/// Slashes that are not yet applied.
	#[codec(compact)]
	pub pending_slash: BalanceOf<T>,
	/// Whether this delegatee is blocked from receiving new delegations.
	pub blocked: bool,
}

impl<T: Config> DelegationRegister<T> {
	/// balance that can be staked.
	pub fn delegated_balance(&self) -> BalanceOf<T> {
		// do not allow to stake more than unapplied slash
		self.total_delegated.saturating_sub(self.pending_slash)
	}

	/// balance that is delegated but not bonded.
	pub fn unbonded_balance(&self) -> BalanceOf<T> {
		self.total_delegated.saturating_sub(self.hold)
	}

	/// consumes self and returns Delegation Register with updated hold amount.
	pub fn update_hold(self, amount: BalanceOf<T>) -> Self {
		DelegationRegister { hold: amount, ..self }
	}
}

impl<T: Config> Delegatee for Pallet<T> {
	type Balance = BalanceOf<T>;
	type AccountId = T::AccountId;

	fn delegated_balance(who: &Self::AccountId) -> Self::Balance {
		<Delegatees<T>>::get(who)
			.map_or_else(|| 0u32.into(), |register| register.delegated_balance())
	}

	fn unbonded_balance(who: &Self::AccountId) -> Self::Balance {
		<Delegatees<T>>::get(who)
			.map_or_else(|| 0u32.into(), |register| register.unbonded_balance())
	}

	fn accept_delegations(
		who: &Self::AccountId,
		reward_destination: &Self::AccountId,
	) -> DispatchResult {
		// Existing delegatee cannot accept delegation
		ensure!(!<Delegatees<T>>::contains_key(who), Error::<T>::NotAllowed);

		// make sure they are not already a direct staker
		ensure!(T::CoreStaking::status(who).is_err(), Error::<T>::AlreadyStaker);

		// payee account cannot be same as delegatee
		ensure!(reward_destination != who, Error::<T>::InvalidRewardDestination);

		// if already a delegator, unblock and return success
		<Delegatees<T>>::mutate(who, |maybe_register| {
			if let Some(register) = maybe_register {
				register.blocked = false;
				register.payee = reward_destination.clone();
			} else {
				*maybe_register = Some(DelegationRegister {
					payee: reward_destination.clone(),
					total_delegated: Zero::zero(),
					hold: Zero::zero(),
					pending_slash: Zero::zero(),
					blocked: false,
				});
			}
		});

		Ok(())
	}

	/// Transfers funds from current staked account to `proxy_delegator`. Current staked account
	/// becomes a delegatee with `proxy_delegator` delegating stakes to it.
	fn migrate_accept_delegations(
		new_delegatee: &Self::AccountId,
		proxy_delegator: &Self::AccountId,
		payee: &Self::AccountId,
	) -> DispatchResult {
		ensure!(new_delegatee != proxy_delegator, Error::<T>::InvalidDelegation);

		// ensure proxy delegator has at least minimum balance to keep the account alive.
		ensure!(
			T::Currency::reducible_balance(
				proxy_delegator,
				Preservation::Expendable,
				Fortitude::Polite
			) > Zero::zero(),
			Error::<T>::NotEnoughFunds
		);

		// ensure staker is a nominator
		let status = T::CoreStaking::status(new_delegatee)?;
		match status {
			StakerStatus::Nominator(_) => (),
			_ => return Err(Error::<T>::InvalidDelegation.into()),
		}

		let stake = T::CoreStaking::stake(new_delegatee)?;

		// unlock funds from staker
		T::CoreStaking::force_unlock(new_delegatee)?;

		// try transferring the staked amount. This should never fail but if it does, it indicates
		// bad state and we abort.
		T::Currency::transfer(
			new_delegatee,
			proxy_delegator,
			stake.total,
			Preservation::Expendable,
		)
		.map_err(|_| Error::<T>::BadState)?;

		// delegate from new delegator to staker.
		Self::accept_delegations(new_delegatee, payee)?;
		Self::delegate(proxy_delegator, new_delegatee, stake.total)
	}

	fn block_delegations(delegatee: &Self::AccountId) -> DispatchResult {
		<Delegatees<T>>::mutate(delegatee, |maybe_register| {
			if let Some(register) = maybe_register {
				register.blocked = true;
				Ok(())
			} else {
				Err(Error::<T>::NotDelegatee.into())
			}
		})
	}

	fn kill_delegatee(delegatee: &Self::AccountId) -> DispatchResult {
		todo!()
	}

	fn update_bond(who: &Self::AccountId) -> DispatchResult {
		let delegatee = <Delegatees<T>>::get(who).ok_or(Error::<T>::NotDelegatee)?;
		let amount_to_bond = delegatee.unbonded_balance();

		match T::CoreStaking::stake(who) {
			Ok(stake) => {
				T::CoreStaking::bond_extra(who, amount_to_bond)
			},
			Err(_) => {
				// If stake not found, it means this is the first bond
				T::CoreStaking::bond(who, amount_to_bond, &delegatee.payee)
			},
		}
	}

	#[transactional]
	fn withdraw(
		delegator: &Self::AccountId,
		delegatee: &Self::AccountId,
		value: Self::Balance,
		num_slashing_spans: u32,
	) -> DispatchResult {
		// fixme(ank4n) handle killing of stash
		let _stash_killed: bool = T::CoreStaking::withdraw_exact(delegatee, value, num_slashing_spans).map_err(|_| Error::<T>::WithdrawFailed)?;
		Self::delegation_withdraw(delegator, delegatee, value)
	}

	fn apply_slash(
		delegatee: &Self::AccountId,
		delegator: &Self::AccountId,
		value: Self::Balance,
		reporter: Option<Self::AccountId>,
	) -> DispatchResult {
		todo!()
	}

	/// Move funds from proxy delegator to actual delegator.
	// TODO: Keep track of proxy delegator and only allow movement from proxy -> new delegator
	#[transactional]
	fn migrate_delegator(
		delegatee: &Self::AccountId,
		existing_delegator: &Self::AccountId,
		new_delegator: &Self::AccountId,
		value: Self::Balance,
	) -> DispatchResult {
		ensure!(value >= T::Currency::minimum_balance(), Error::<T>::NotEnoughFunds);

		// ensure delegatee exists.
		ensure!(!<Delegatees<T>>::contains_key(delegatee), Error::<T>::NotDelegatee);

		// remove delegation of `value` from `existing_delegator`.
		Self::delegation_withdraw(existing_delegator, delegatee, value)?;

		// transfer the withdrawn value to `new_delegator`.
		T::Currency::transfer(existing_delegator, new_delegator, value, Preservation::Expendable)
			.map_err(|_| Error::<T>::BadState)?;

		// add the above removed delegation to `new_delegator`.
		Self::delegate(new_delegator, delegatee, value)
	}
}

impl<T: Config> Delegator for Pallet<T> {
	type Balance = BalanceOf<T>;
	type AccountId = T::AccountId;

	fn delegate(
		delegator: &Self::AccountId,
		delegatee: &Self::AccountId,
		value: Self::Balance,
	) -> DispatchResult {
		let delegator_balance =
			T::Currency::reducible_balance(&delegator, Preservation::Expendable, Fortitude::Polite);
		ensure!(value >= T::Currency::minimum_balance(), Error::<T>::NotEnoughFunds);
		ensure!(delegator_balance >= value, Error::<T>::NotEnoughFunds);
		ensure!(delegatee != delegator, Error::<T>::InvalidDelegation);
		ensure!(<Delegatees<T>>::contains_key(delegatee), Error::<T>::NotDelegatee);

		// cannot delegate to another delegatee.
		if <Delegatees<T>>::contains_key(delegator) {
			return Err(Error::<T>::InvalidDelegation.into())
		}

		let new_delegation_amount = if let Some((current_delegatee, current_delegation)) =
			<Delegators<T>>::get(delegator)
		{
			ensure!(&current_delegatee == delegatee, Error::<T>::InvalidDelegation);
			value.saturating_add(current_delegation)
		} else {
			value
		};

		<Delegators<T>>::insert(delegator, (delegatee, new_delegation_amount));
		<Delegatees<T>>::mutate(delegatee, |maybe_register| {
			if let Some(register) = maybe_register {
				register.total_delegated.saturating_accrue(value);
			}
		});

		T::Currency::hold(&HoldReason::Delegating.into(), &delegator, value)?;

		Ok(())
	}

	fn unbond(delegatee: &Self::AccountId, value: Self::Balance) -> DispatchResult {
		T::CoreStaking::unbond(delegatee, value)
	}
}

impl<T: Config> sp_staking::StakingHoldProvider for Pallet<T> {
	type Balance = BalanceOf<T>;
	type AccountId = T::AccountId;

	fn update_hold(who: &Self::AccountId, amount: Self::Balance) -> DispatchResult {
		if !Self::is_delegatee(who) {
			return T::CoreStaking::update_hold(who, amount);
		}

		// delegation register should exist since `who` is a delegatee.
		let delegation_register =
			<Delegatees<T>>::get(who).defensive_ok_or(Error::<T>::BadState)?;

		ensure!(delegation_register.total_delegated >= amount, Error::<T>::NotEnoughFunds);
		ensure!(delegation_register.pending_slash <= amount, Error::<T>::UnappliedSlash);
		let updated_register = delegation_register.update_hold(amount);
		<Delegatees<T>>::insert(who, updated_register);

		Ok(())
	}

	fn release(who: &Self::AccountId) {
		if !Self::is_delegatee(who) {
			T::CoreStaking::release(who);
		}

		let _delegation_register = <Delegatees<T>>::get(who);
		todo!("handle kill delegatee")
	}
}
impl<T: Config> StakingDelegationSupport for Pallet<T> {
	fn stakeable_balance(who: &Self::AccountId) -> Self::Balance {
		<Delegatees<T>>::get(who).map_or_else(
			|| T::Currency::reducible_balance(who, Preservation::Expendable, Fortitude::Polite),
			|delegatee| delegatee.delegated_balance(),
		)
	}

	fn restrict_reward_destination(
		who: &Self::AccountId,
		reward_destination: Option<Self::AccountId>,
	) -> bool {
		let maybe_register = <Delegatees<T>>::get(who);

		if maybe_register.is_none() {
			// no restrictions for non delegatees.
			return false;
		}

		// restrict if reward destination is not set
		if reward_destination.is_none() {
			return true;
		}

		let register = maybe_register.expect("checked above; qed");
		let reward_acc = reward_destination.expect("checked above; qed");

		// restrict if reward account is not what delegatee registered.
		register.payee != reward_acc
	}

	#[cfg(feature = "std")]
	fn stake_type(who: &Self::AccountId) -> StakeBalanceType {
		if !Self::is_delegatee(who) {
			return StakeBalanceType::Direct;
		}

		StakeBalanceType::Delegated
	}
}

impl<T: Config> Pallet<T> {
	fn is_delegatee(who: &T::AccountId) -> bool {
		<Delegatees<T>>::contains_key(who)
	}

	fn delegation_withdraw(
		delegator: &T::AccountId,
		delegatee: &T::AccountId,
		value: BalanceOf<T>,
	) -> DispatchResult {
		<Delegators<T>>::mutate_exists(delegator, |maybe_delegate| match maybe_delegate {
			Some((current_delegatee, delegate_balance)) => {
				ensure!(&current_delegatee.clone() == delegatee, Error::<T>::NotDelegatee);
				ensure!(*delegate_balance >= value, Error::<T>::NotEnoughFunds);

				delegate_balance.saturating_reduce(value);

				if *delegate_balance == BalanceOf::<T>::zero() {
					*maybe_delegate = None;
				}
				Ok(())
			},
			None => {
				// delegator does not exist
				return Err(Error::<T>::NotDelegator)
			},
		})?;

		<Delegatees<T>>::mutate(delegatee, |maybe_register| match maybe_register {
			Some(ledger) => {
				ledger.total_delegated.saturating_reduce(value);
				Ok(())
			},
			None => {
				// Delegatee not found
				return Err(Error::<T>::NotDelegatee)
			},
		})?;

		let released = T::Currency::release(
			&HoldReason::Delegating.into(),
			&delegator,
			value,
			Precision::BestEffort,
		)?;

		defensive_assert!(released == value, "hold should have been released fully");

		Ok(())
	}
}

#[cfg(any(test, feature = "try-runtime"))]
impl<T: Config> Pallet<T> {
	pub(crate) fn do_try_state() -> Result<(), sp_runtime::TryRuntimeError> {
		Ok(())
	}
}