use crate::gadgets;
use crate::prelude::*;

use clap::Parser;

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
                ext: &mut Ext
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

macro_rules! min_active_stake_for {
    ($runtime:ident) => {
        paste::paste! {
            pub(crate) fn [<min_active_stake_ $runtime>]<T: EPM::Config>(
                ext: &mut Ext
            ) -> Result<(), anyhow::Error> {
                use $crate::[<$runtime _runtime_exports>]::*;

                log::info!(target: LOG_TARGET, "Transform::min_active_stake starting.");

                let min_active_stake = gadgets::min_active_stake::<Runtime>(ext);

                log::info!(target: LOG_TARGET, "Transform::min_active_stake result {}", min_active_stake);

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
