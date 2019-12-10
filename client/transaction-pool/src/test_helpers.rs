// Copyright 2018-2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.
use super::*;

use codec::Encode;
use futures::executor::block_on;
use txpool::{self, Pool};
use test_client::{
	runtime::{
		AccountId, Block, Hash, Index, Extrinsic, Transfer
	},
	AccountKeyring::{self, *}
};
use sp_runtime::{
	generic::{self, BlockId},
	traits::{Hash as HashT, BlakeTwo256},
	transaction_validity::{TransactionValidity, ValidTransaction},
};

pub struct TestApi {
	pub modifier: Box<dyn Fn(&mut ValidTransaction) + Send + Sync>,
}

impl TestApi {
	pub fn default() -> Self {
		TestApi {
			modifier: Box::new(|_| {}),
		}
	}
}

impl txpool::ChainApi for TestApi {
	type Block = Block;
	type Hash = Hash;
	type Error = error::Error;
	type ValidationFuture = futures::future::Ready<error::Result<TransactionValidity>>;

	fn validate_transaction(
		&self,
		at: &BlockId<Self::Block>,
		uxt: txpool::ExtrinsicFor<Self>,
	) -> Self::ValidationFuture {
		let expected = index(at);
		let requires = if expected == uxt.transfer().nonce {
			vec![]
		} else {
			vec![vec![uxt.transfer().nonce as u8 - 1]]
		};
		let provides = vec![vec![uxt.transfer().nonce as u8]];

		let mut validity = ValidTransaction {
			priority: 1,
			requires,
			provides,
			longevity: 64,
			propagate: true,
		};

		(self.modifier)(&mut validity);

		futures::future::ready(Ok(
			Ok(validity)
		))
	}

	fn block_id_to_number(&self, at: &BlockId<Self::Block>) -> error::Result<Option<txpool::NumberFor<Self>>> {
		Ok(Some(number_of(at)))
	}

	fn block_id_to_hash(&self, at: &BlockId<Self::Block>) -> error::Result<Option<txpool::BlockHash<Self>>> {
		Ok(match at {
			generic::BlockId::Hash(x) => Some(x.clone()),
			_ => Some(Default::default()),
		})
	}

	fn hash_and_length(&self, ex: &txpool::ExtrinsicFor<Self>) -> (Self::Hash, usize) {
		let encoded = ex.encode();
		(BlakeTwo256::hash(&encoded), encoded.len())
	}

}

fn index(at: &BlockId<Block>) -> u64 {
	209 + number_of(at)
}

fn number_of(at: &BlockId<Block>) -> u64 {
	match at {
		generic::BlockId::Number(n) => *n as u64,
		_ => 0,
	}
}

pub fn uxt(who: AccountKeyring, nonce: Index) -> Extrinsic {
	let transfer = Transfer {
		from: who.into(),
		to: AccountId::default(),
		nonce,
		amount: 1,
	};
	let signature = transfer.using_encoded(|e| who.sign(e));
	Extrinsic::Transfer(transfer, signature.into())
}

pub fn pool() -> Pool<TestApi> {
	Pool::new(Default::default(), TestApi::default())
}
