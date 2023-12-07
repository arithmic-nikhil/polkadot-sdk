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

//! Generic implementation of a block arithmic transactions.

use crate::{
	codec::{Codec, Decode, Encode},
	generic::Digest,
	scale_info::TypeInfo,
	traits::{
		self, AtLeast32BitUnsigned, Hash as HashT, MaybeDisplay, MaybeFromStr,
		MaybeSerializeDeserialize, Member,
	},
};
use codec::{FullCodec, MaxEncodedLen};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use sp_core::U256;
use sp_std::fmt::Debug;

use crate::Vec;

/// Abstraction over a block arithmic transactions for a substrate chain.
#[derive(Encode, Decode, PartialEq, Eq, Clone, sp_core::RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(Hash))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct ArithmicTransactions {
	/// transactions
	pub transactions: Vec<(u128)>,
}

#[cfg(feature = "serde")]
pub fn serialize_number<S, T: Copy + Into<U256> + TryFrom<U256>>(
	val: &T,
	s: S,
) -> Result<S::Ok, S::Error>
where
	S: serde::Serializer,
{
	let u256: U256 = (*val).into();
	serde::Serialize::serialize(&u256, s)
}

#[cfg(feature = "serde")]
pub fn deserialize_number<'a, D, T: Copy + Into<U256> + TryFrom<U256>>(d: D) -> Result<T, D::Error>
where
	D: serde::Deserializer<'a>,
{
	let u256: U256 = serde::Deserialize::deserialize(d)?;
	TryFrom::try_from(u256).map_err(|_| serde::de::Error::custom("Try from failed"))
}

impl traits::ArithmicTransactions for ArithmicTransactions {

	fn new(
		transactions: Vec<(u128)>,
	) -> Self {
		Self { transactions }
	}
	fn transactions(&self) -> &Vec<(u128)> {
		&self.transactions
	}
}

#[cfg(all(test, feature = "std"))]
mod tests {
	// TODO: Implement test cases here
}