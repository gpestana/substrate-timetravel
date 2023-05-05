use std::marker::PhantomData;

use crate::gadgets;
use crate::prelude::*;

use EPM::BalanceOf;

use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Parser)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum Operation {
    /// Calculates the staking minimum active stake.
    MinActiveStake,

    /// Performs analysys of the election and staking data.
    ElectionAnalysis,
}

macro_rules! election_analysis_for {
    ($runtime:ident) => {
        paste::paste! {
            pub(crate) fn [<election_analysis_ $runtime>]<T: EPM::Config>(
                ext: &mut Ext,
                output_path: String,
            ) -> Result<(), anyhow::Error> {
                use $crate::[<$runtime _runtime_exports>]::*;

                log::info!(target: LOG_TARGET, "Transform::election_analysis starting.");

                let (snapshot_metadata, snapshot_size) = gadgets::snapshot_data_or_force::<Runtime>(ext);

                let min_active_stake = gadgets::min_active_stake::<Runtime>(ext);
                let block_number = gadgets::block_number::<Runtime>(ext);

                let dpos_score = gadgets::mine_dpos::<Runtime>(ext)?;

                Ok(())
            }
        }
    };
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MinActiveStakeCsv {
    block_number: u32,
    min_active_stake: u128,
}

macro_rules! min_active_stake_for {
    ($runtime:ident) => {
        paste::paste! {
            pub(crate) fn [<min_active_stake_ $runtime>]<T: EPM::Config>(
                ext: &mut Ext,
                output_path: String,
            ) -> Result<(), anyhow::Error> {
                use $crate::[<$runtime _runtime_exports>]::*;

                log::info!(target: LOG_TARGET, "Transform::min_active_stake starting.");

                let min_active_stake = gadgets::min_active_stake::<Runtime>(ext);
                let block_number = gadgets::block_number::<Runtime>(ext);

                let csv_entry = MinActiveStakeCsv {
                    block_number,
                    min_active_stake,
                };

                crate::write_csv(csv_entry, &output_path)?;

                log::info!(
                    target: LOG_TARGET,
                    "Transform::min_active_stake result {}; CSV entry stored in {:?}",
                    min_active_stake,
                    output_path
                );

                Ok(())
            }
        }
    };
}

election_analysis_for!(polkadot);
election_analysis_for!(kusama);
election_analysis_for!(westend);

min_active_stake_for!(polkadot);
min_active_stake_for!(kusama);
min_active_stake_for!(westend);
