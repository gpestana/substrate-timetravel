use crate::configs::{ExtractConfig, TransformConfig};
use crate::gadgets;
use crate::operations::Operation;
use crate::Error;

use anyhow::anyhow;

use frame_support::{storage::generator::StorageMap, traits::PalletInfo};
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
			)  -> Result<(), anyhow::Error> {
				use $crate::[<$runtime _runtime_exports>]::*;

            	log::info!(target: LOG_TARGET, "Scrapping keys for pallets {:?} in block {:?}", pallets, block_hash);

				Builder::<Block>::new()
					.mode(Mode::Online(OnlineConfig {
						transport: Transport::Uri(uri),
						at: Some(block_hash),
						pallets,
						hashed_prefixes: vec![<frame_system::BlockHash<Runtime>>::prefix_hash()],
						hashed_keys: vec![[twox_128(b"System"), twox_128(b"Number")].concat()],
						state_snapshot: Some(snapshot_path.clone().into()),
						..Default::default()
					}))
					.build()
                    .await
                    .map_err(|e| return anyhow!(Error::Externalities{ error: e.to_string()}))?;

				log::info!(target: LOG_TARGET, "Extract done, snapshot stored in {:?}", snapshot_path);

                Ok(())
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
            )  -> Result<(), anyhow::Error> {
                use $crate::[<$runtime _runtime_exports>]::*;

                let mut ext = Builder::<Block>::new()
                    .mode(Mode::Offline(OfflineConfig {
				        state_snapshot: SnapshotConfig::new(snapshot_path.clone()),
                }))
                .build()
                .await
                .map_err(|e| return anyhow!(Error::Externalities{ error: e.to_string()}))?;

                log::info!(target: LOG_TARGET, "Loaded snapshot from {:?}", snapshot_path);

                match operation {
                    Operation::MinActiveStake => crate::operations::[<min_active_stake_ $runtime>]::<Runtime>(&mut ext),
                    Operation::ElectionAnalysis => crate::operations::[<election_analysis_ $runtime>]::<Runtime>(&mut ext),
                }
            }
        }
    };
}

extract_for!(polkadot);
extract_for!(kusama);
extract_for!(westend);

transform_for!(polkadot);
transform_for!(kusama);
transform_for!(westend);
