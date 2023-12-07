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

//! Generic implementation of a block and associated items.

#[cfg(feature = "std")]
use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
	codec::{Codec, Decode, Encode},
	traits::{
		self, Block as BlockT, Header as HeaderT,ArithmicTransactions as ArithmicTransactionsT, MaybeSerialize, MaybeSerializeDeserialize,
		Member, NumberFor,
	},
	Justifications,
};
use sp_core::RuntimeDebug;
use sp_std::prelude::*;

/// Something to identify a block.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum BlockId<Block: BlockT> {
	/// Identify by block header hash.
	Hash(Block::Hash),
	/// Identify by block number.
	Number(NumberFor<Block>),
}

impl<Block: BlockT> BlockId<Block> {
	/// Create a block ID from a hash.
	pub const fn hash(hash: Block::Hash) -> Self {
		BlockId::Hash(hash)
	}

	/// Create a block ID from a number.
	pub const fn number(number: NumberFor<Block>) -> Self {
		BlockId::Number(number)
	}

	/// Check if this block ID refers to the pre-genesis state.
	pub fn is_pre_genesis(&self) -> bool {
		match self {
			BlockId::Hash(hash) => hash == &Default::default(),
			BlockId::Number(_) => false,
		}
	}

	/// Create a block ID for a pre-genesis state.
	pub fn pre_genesis() -> Self {
		BlockId::Hash(Default::default())
	}
}

impl<Block: BlockT> Copy for BlockId<Block> {}

#[cfg(feature = "std")]
impl<Block: BlockT> fmt::Display for BlockId<Block> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

/// Abstraction over a substrate block.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct Block<Header, Extrinsic, ArithmicTransactions> {
	/// The block header.
	pub header: Header,
	/// The accompanying extrinsics.
	pub extrinsics: Vec<Extrinsic>,
	/// The accompanying transactions
	pub arithmic_transactions: ArithmicTransactions,
}

impl<Header, Extrinsic, ArithmicTransactions> traits::HeaderProvider for Block<Header, Extrinsic, ArithmicTransactions>
where
	Header: HeaderT,
{
	type HeaderT = Header;
}

impl<Header, Extrinsic, ArithmicTransactions> traits::ArithmicTransactionsProvider for Block<Header, Extrinsic, ArithmicTransactions>
	where
		ArithmicTransactions: ArithmicTransactionsT,
{
	type ArithmicTransactionsT = ArithmicTransactions;
}

impl<Header, Extrinsic: MaybeSerialize, ArithmicTransactions> traits::Block for Block<Header, Extrinsic, ArithmicTransactions>
where
	Header: HeaderT + MaybeSerializeDeserialize,
	Extrinsic: Member + Codec + traits::Extrinsic,
	ArithmicTransactions: ArithmicTransactionsT + MaybeSerializeDeserialize,
{
	type Extrinsic = Extrinsic;
	type Header = Header;
	type ArithmicTransactions = ArithmicTransactions;
	type Hash = <Self::Header as traits::Header>::Hash;

	fn header(&self) -> &Self::Header {
		&self.header
	}
	fn extrinsics(&self) -> &[Self::Extrinsic] {
		&self.extrinsics[..]
	}
	fn arithmic_transactions(&self) -> &Self::ArithmicTransactions {
		&self.arithmic_transactions
	}
	fn deconstruct(self) -> (Self::Header, Vec<Self::Extrinsic>, Self::ArithmicTransactions) {
		(self.header, self.extrinsics, self.arithmic_transactions)
	}
	fn new(header: Self::Header, extrinsics: Vec<Self::Extrinsic>, arithmic_transactions: Self::ArithmicTransactions) -> Self {
		Block { header, extrinsics, arithmic_transactions }
	}
	fn encode_from(header: &Self::Header, extrinsics: &[Self::Extrinsic], arithmic_transactions: &Self::ArithmicTransactions) -> Vec<u8> {
		(header, extrinsics, arithmic_transactions).encode()
	}
}

/// Abstraction over a substrate block and justification.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct SignedBlock<Block> {
	/// Full block.
	pub block: Block,
	/// Block justification.
	pub justifications: Option<Justifications>,
}
