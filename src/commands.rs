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
                block_hash: H256,
                snapshot_path: String,
                live: bool,
			)  -> Result<Ext, anyhow::Error> {
				use $crate::[<$runtime _runtime_exports>]::*;

            	log::info!(target: LOG_TARGET, "Scrapping keys for pallets {:?} in block {:?}", pallets, block_hash);

                let state_snapshot = if live { None } else { Some(snapshot_path.clone().into())};

				let ext = Builder::<Block>::new()
					.mode(Mode::Online(OnlineConfig {
						transport: Transport::Uri(uri),
						at: Some(block_hash),
						pallets,
						hashed_prefixes: vec![<frame_system::BlockHash<Runtime>>::prefix_hash().to_vec()],
						hashed_keys: vec![[twox_128(b"System"), twox_128(b"Number")].concat()],
						state_snapshot,
						..Default::default()
					}))
					.build()
                    .await
		            .map(|rx| rx.inner_ext)
                    .map_err(|e| return anyhow!(Error::Externalities{ error: e.to_string()}))?;

                match live {
                    false => log::info!(target: LOG_TARGET, "Extract done, snapshot stored in {:?}", snapshot_path),
                    true => log::info!(target: LOG_TARGET, "Extract done, snapshot not stored"),
                };

                Ok(ext)
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
                block_hash: H256,
                output_path: String,
                snapshot_path: String,
                compute_unbounded: bool,
                live: bool,
            )  -> Result<(), anyhow::Error> {
                use $crate::[<$runtime _runtime_exports>]::*;

                let mut ext = if live {
                    let default_pallets = vec!["ElectionProviderMultiPhase".to_string(), "Staking".to_string(), "VoterList".to_string()];
                    let ext = extract_cmd(uri, default_pallets, block_hash, snapshot_path.clone(), true).await?;
                    ext
                } else {

                    Builder::<Block>::new()
                        .mode(Mode::Offline(OfflineConfig {
				        state_snapshot: SnapshotConfig::new(snapshot_path.clone()),
                    }))
                    .build()
                    .await
		            .map(|rx| rx.inner_ext)
                    .map_err(|e| return anyhow!(Error::Externalities{ error: e.to_string()}))?
                };

                log::info!(target: LOG_TARGET, "Loaded snapshot from {:?}", snapshot_path);

                match operation {
                    Operation::MinActiveStake => crate::operations::[<min_active_stake_ $runtime>]::<Runtime>(&mut ext, output_path),
                    Operation::ElectionAnalysis => crate::operations::[<election_analysis_ $runtime>]::<Runtime>(&mut ext, output_path, compute_unbounded),
                    Operation::Playground => crate::operations::[<playground_ $runtime>]::<Runtime>(&mut ext),
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
