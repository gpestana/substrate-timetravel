//! Main commands of `substrate-timetravel` CLI
//!
//! The commands are split into two main branches: `extract` and `transform`:
//! * `substrat-timetravel extract`: fetches the keys of a given tuple {`block`, `pallets`} using
//! [`frame_remote_externalities`](https://paritytech.github.io/substrate/master/frame_remote_externalities/index.html)
//! and stores the externalities snapshot in disk for posterior use.
//! * `subtrate-timetravel transform`: computes a given transformation on an externalities and stored the
//! results in a CSV file.

use crate::operations::Operation;
use crate::prelude::*;
use crate::Error;

use anyhow::anyhow;

use frame_support::storage::generator::StorageMap;
use remote_externalities::{Builder, Mode, OfflineConfig, OnlineConfig, SnapshotConfig, Transport};
use sp_core::{hashing::twox_128, H256};

macro_rules! extract_for {
	($runtime:ident) => {
		paste::paste! {
			pub(crate) async fn [<extract_cmd_ $runtime>](
				uri: String,
                pallets: Vec<String>,
                block_hashes: Vec<H256>,
                snapshot_paths: Vec<String>,
                live: bool,
			)  -> Result<Vec<Ext>, anyhow::Error> {
				use $crate::[<$runtime _runtime_exports>]::*;

                log::info!(target: LOG_TARGET, "Scrapping keys for pallets {:?} for block(s) {:?}", pallets, block_hashes);

                let mut exts: Vec<Ext> = vec![];

                for (i, block_hash) in block_hashes.iter().enumerate() {
                    let state_snapshot = if live { None } else { Some(snapshot_paths[i].clone().into()) };

                    let ext = Builder::<Block>::new()
					    .mode(Mode::Online(OnlineConfig {
						transport: Transport::Uri(uri.clone()),
						at: Some(*block_hash),
						pallets: pallets.clone(),
						hashed_prefixes: vec![<frame_system::BlockHash<Runtime>>::prefix_hash().to_vec()],
						hashed_keys: vec![[twox_128(b"System"), twox_128(b"Number")].concat()],
						state_snapshot,
						..Default::default()
					}))
					.build()
                    .await
		            .map(|rx| rx.inner_ext)
                    .map_err(|e| return anyhow!(Error::Externalities{ error: e.to_string()}))?;

                    exts.push(ext);
                }

                log::info!(target: LOG_TARGET, "Extract done, snapshot(s) stored in {:?}", snapshot_paths);

                Ok(exts)
			}
		}
	};
}

macro_rules! transform_for {
    ($runtime:ident) => {
        paste::paste! {
            pub(crate) async fn [<transform_cmd_ $runtime>](
                uri: String,
                operation: Operation,
                block_hashes: Vec<H256>,
                output_path: String,
                snapshot_paths: Vec<String>,
                compute_unbounded: bool,
                live: bool,
            )  -> Result<(), anyhow::Error> {
                use $crate::[<$runtime _runtime_exports>]::*;

                let exts = if live {
                    let default_pallets = vec!["ElectionProviderMultiPhase".to_string(), "Staking".to_string(), "VoterList".to_string()];
                    extract_cmd(uri, default_pallets, block_hashes, snapshot_paths.clone(), true).await?
                } else {
                    let mut exts = vec![];

                    for snapshot_path in snapshot_paths.clone() {
                        let ext = Builder::<Block>::new()
                            .mode(Mode::Offline(OfflineConfig {
				            state_snapshot: SnapshotConfig::new(snapshot_path)
                        }))
                        .build()
                        .await
		                .map(|rx| rx.inner_ext)
                        .map_err(|e| return anyhow!(Error::Externalities{ error: e.to_string()}))?;

                        exts.push(ext);
                    }
                    exts
                };

                log::info!(target: LOG_TARGET, "Loaded snapshot from {:?}", snapshot_paths);

                match operation {
                    Operation::MinActiveStake => crate::operations::[<min_active_stake_ $runtime>]::<Runtime>(exts, output_path),
                    Operation::ElectionAnalysis => crate::operations::[<election_analysis_ $runtime>]::<Runtime>(exts, output_path, compute_unbounded),
                    Operation::StakingLedgerChecks => crate::operations::[<staking_ledger_checks_ $runtime>]::<Runtime>(exts),
                    Operation::Playground => crate::operations::[<playground_ $runtime>]::<Runtime>(exts),
                }
            }
        }
    };
}

//extract_for!(polkadot);
//extract_for!(kusama);
extract_for!(westend);

//transform_for!(polkadot);
//transform_for!(kusama);
transform_for!(westend);
