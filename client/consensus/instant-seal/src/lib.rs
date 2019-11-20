// Copyright 2019 Parity Technologies (UK) Ltd.
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

//! An instant sealing engine: we always author on every slot, and accept all blocks.
//! This is suitable for single-node development environments.

use consensus_common::{
	self, BlockImport, Environment, Proposer, BlockCheckParams,
	ForkChoiceStrategy, BlockImportParams, BlockOrigin,
	ImportResult, SelectChain,
};
use consensus_common::import_queue::{BasicQueue, CacheKeyId, Verifier, BoxBlockImport};
use sr_primitives::traits::{Block as BlockT};
use sr_primitives::Justification;
use parking_lot::Mutex;
use futures::prelude::*;
use transaction_pool::txpool::{self, Pool as TransactionPool};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// The synchronous block-import worker of the engine.
pub struct InstantSealBlockImport<I> {
	inner: I,
}

impl<I> From<I> for InstantSealBlockImport<I> {
	fn from(i: I) -> Self {
		InstantSealBlockImport { inner: i }
	}
}

impl<B: BlockT, I: BlockImport<B>> BlockImport<B> for InstantSealBlockImport<I> {
	type Error = I::Error;

	fn check_block(&mut self, block: BlockCheckParams<B>) -> Result<ImportResult, Self::Error>
	{
		self.inner.check_block(block)
	}

	fn import_block(
		&mut self,
		block: BlockImportParams<B>,
		cache: HashMap<CacheKeyId, Vec<u8>>,
	) -> Result<ImportResult, Self::Error> {
		// TODO: strip out post-digest.

		self.inner.import_block(block, cache)
	}
}

/// The verifier for the instant seal engine; instantly finalizes.
struct InstantSealVerifier;

impl<B: BlockT> Verifier<B> for InstantSealVerifier {
	fn verify(
		&mut self,
		origin: BlockOrigin,
		header: B::Header,
		justification: Option<Justification>,
		body: Option<Vec<B::Extrinsic>>,
	) -> Result<(BlockImportParams<B>, Option<Vec<(CacheKeyId, Vec<u8>)>>), String> {
		let import_params = BlockImportParams {
			origin,
			header,
			justification,
			post_digests: Vec::new(),
			body,
			finalized: true,
			auxiliary: Vec::new(),
			fork_choice: ForkChoiceStrategy::LongestChain,
			allow_missing_state: false,
		};

		Ok((import_params, None))
	}
}

/// Instantiate the import queue for the instant-seal consensus engine.
pub fn import_queue<B: BlockT>(block_import: BoxBlockImport<B>) -> BasicQueue<B>
{
	BasicQueue::new(
		InstantSealVerifier,
		block_import,
		None,
		None,
	)
}

/// Creates the background authorship task for the instant seal engine.
pub async fn run_instant_seal<B, E, A, C>(
	block_import: BoxBlockImport<B>,
	env: E,
	pool: Arc<TransactionPool<A>>,
	select_chain: C,
	inherent_data_providers: inherents::InherentDataProviders,
)
	where
		B: BlockT + 'static,
		E: Environment<B> + 'static,
		A: txpool::ChainApi + 'static,
		C: SelectChain<B> + 'static,
{
	let block_import = Arc::new(Mutex::new(block_import));
	let env = Arc::new(Mutex::new(env));
	let select_chain = Arc::new(select_chain);
	let inherent_data_providers = Arc::new(inherent_data_providers);
	let moved_pool = pool.clone();

	// propose a new block everytime a transaction is imported
	pool.import_notification_stream()
		.for_each(move |_| {
			let select_chain = select_chain.clone();
			let env = env.clone();
			let inherent_data_providers = inherent_data_providers.clone();
			let block_import = block_import.clone();
			let moved_pool = moved_pool.clone();

			async move {
				// prev
				if moved_pool.status().ready == 0 {
					return
				}

				let best_block_header = match select_chain.clone().best_chain() {
					Err(_) => return,
					Ok(best) => best,
				};

				let mut proposer = match env.clone().lock().init(&best_block_header) {
					Err(_) => return,
					Ok(p) => p,
				};

				let id = match inherent_data_providers.clone().create_inherent_data() {
					Err(_) => return,
					Ok(id) => id,
				};

				let result = proposer.propose(
					id,
					Default::default(),
					Duration::from_secs(5),
				).await;

				match result {
					Ok(block) => {
						let (header, body) = block.deconstruct();
						let import_params = BlockImportParams {
							origin: BlockOrigin::Own,
							header,
							justification: None,
							post_digests: Vec::new(),
							body: Some(body),
							finalized: true,
							auxiliary: Vec::new(),
							fork_choice: ForkChoiceStrategy::LongestChain,
							allow_missing_state: false,
						};

						let res = block_import.clone()
							.lock()
							.import_block(import_params, HashMap::new());
						if let Err(e) = res {
							log::warn!("Failed to import just-constructed block: {:?}", e);
						}
					}
					Err(e) => {
						log::warn!("Failed to propose block: {:?}", e)
					}
				};
			}
		}).await
}
