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

//! A manual sealing engine: the engine listens for rpc calls to seal blocks and create forks
//! This is suitable for a testing environment.

use consensus_common::{
	self, BlockImport, Environment, Proposer, BlockCheckParams,
	ForkChoiceStrategy, BlockImportParams, BlockOrigin,
	ImportResult, SelectChain,
};
use consensus_common::import_queue::{BasicQueue, CacheKeyId, Verifier, BoxBlockImport};
use sr_primitives::traits::Block as BlockT;
use client::blockchain::HeaderBackend;
use sr_primitives::Justification;
use parking_lot::Mutex;
use futures::prelude::*;
use transaction_pool::txpool::{self, Pool as TransactionPool};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

pub mod rpc;

use rpc::EngineCommand;
use sr_api::BlockId;

/// The synchronous block-import worker of the engine.
pub struct ManualSealBlockImport<I> {
	inner: I,
}

impl<I> From<I> for ManualSealBlockImport<I> {
	fn from(i: I) -> Self {
		ManualSealBlockImport { inner: i }
	}
}

impl<B: BlockT, I: BlockImport<B>> BlockImport<B> for ManualSealBlockImport<I> {
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

/// The verifier for the manual seal engine; instantly finalizes.
struct ManualSealVerifier;

impl<B: BlockT> Verifier<B> for ManualSealVerifier {
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

/// Instantiate the import queue for the manual seal consensus engine.
pub fn import_queue<B: BlockT>(block_import: BoxBlockImport<B>) -> BasicQueue<B>
{
	BasicQueue::new(
		ManualSealVerifier,
		block_import,
		None,
		None,
	)
}

/// Creates the background authorship task for the manual seal engine.
pub async fn run_manual_seal<B, HB, E, A, C, S>(
	block_import: BoxBlockImport<B>,
	env: E,
	back_end: HB,
	pool: Arc<TransactionPool<A>>,
	mut seal_block_channel: S,
	select_chain: C,
	inherent_data_providers: inherents::InherentDataProviders,
)
	where
		B: BlockT + 'static,
		HB: HeaderBackend<B> + 'static,
		E: Environment<B> + 'static,
		A: txpool::ChainApi + 'static,
		S: Stream<Item=EngineCommand<<B as BlockT>::Hash>> + Unpin + 'static,
		C: SelectChain<B> + 'static,
{
	let block_import = Arc::new(Mutex::new(block_import));
	let env = Arc::new(Mutex::new(env));
	let select_chain = Arc::new(select_chain);
	let inherent_data_providers = Arc::new(inherent_data_providers);
	let moved_pool = pool.clone();
	let back_end = Arc::new(back_end);

	while let Some(command) = seal_block_channel.next().await {
		let select_chain = select_chain.clone();
		let env = env.clone();
		let inherent_data_providers = inherent_data_providers.clone();
		let block_import = block_import.clone();
		let moved_pool = moved_pool.clone();
		let back_end = back_end.clone();

		match command {
			EngineCommand::SealNewBlock {
				create_empty,
				parent_hash
			} => {
				if moved_pool.status().ready == 0 && !create_empty {
					return;
				}

				// get the header to build this new block on
				// use the parent_hash supplied via `EngineCommand`
				// or fetch the best_block.
				let header = parent_hash
					.and_then(|hash| {
						back_end.header(BlockId::Hash(hash)).ok()
					})
					.and_then(std::convert::identity)
					.or_else(|| select_chain.best_chain().ok());

				let header = match header {
					None => return,
					Some(hash) => hash,
				};

				let mut proposer = match env.lock().init(&header) {
					Err(_) => return,
					Ok(p) => p,
				};

				let id = match inherent_data_providers.create_inherent_data() {
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

						let res = block_import.lock()
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
		}
	}
}

pub async fn run_instant_seal<B, HB, E, A, C, S>(
	block_import: BoxBlockImport<B>,
	env: E,
	back_end: HB,
	pool: Arc<TransactionPool<A>>,
	select_chain: C,
	inherent_data_providers: inherents::InherentDataProviders,
)
	where
		B: BlockT + 'static,
		HB: HeaderBackend<B> + 'static,
		E: Environment<B> + 'static,
		A: txpool::ChainApi + 'static,
		S: Stream<Item=EngineCommand<<B as BlockT>::Hash>> + 'static,
		C: SelectChain<B> + 'static
{
	// instant-seal creates blocks as soon as transactions are imported
	// into the transaction pool.
	let seal_block_channel = pool.import_notification_stream()
		.map(|_| {
			EngineCommand::SealNewBlock {
				create_empty: false,
				parent_hash: None,
			}
		});

	run_manual_seal(
		block_import,
		env,
		back_end,
		pool,
		seal_block_channel,
		select_chain,
		inherent_data_providers,
	).await
}
