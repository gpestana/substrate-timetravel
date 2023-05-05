use crate::configs::Solver;
use crate::gadgets;
use crate::prelude::*;
use sp_npos_elections::ElectionScore;

use EPM::{BalanceOf, SolutionOrSnapshotSize};

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

#[derive(Debug, Clone, Parser)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum Operation {
    /// Calculates the staking minimum active stake.
    MinActiveStake,

    /// Performs analysys of the election and staking data.
    ElectionAnalysis,
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

#[derive(Debug, Default, Serialize, Deserialize)]
struct ElectionEntryCSV<T: EPM::Config> {
    block_number: u32,
    phrag_min_stake: u128,
    phrag_sum_stake: u128,
    phrag_sum_stake_squared: u128,
    phrag_mms_min_stake: u128,
    phrag_mms_sum_stake: u128,
    phrag_mms_sum_stake_squared: u128,
    dpos_min_stake: u128,
    dpos_sum_stake: u128,
    dpos_sum_stake_squared: u128,
    dpos_unbound_min_stake: u128,
    dpos_unbound_sum_stake: u128,
    dpos_unbound_sum_stake_squared: u128,
    voters: u32,
    targets: u32,
    snapshot_size: usize,
    voters_unbound: u32,
    targets_unbound: u32,
    snapshot_size_unbound: usize,
    min_active_stake: u128,
    #[serde(skip)]
    _marker: PhantomData<T>,
}

// TODO: use? refactor?
use frame_system::pallet_prelude::BlockNumberFor;

impl<T: EPM::Config> ElectionEntryCSV<T> {
    fn new(
        block_number: BlockNumberFor<T>,
        phrag_solutions: (
            &EPM::RawSolution<EPM::SolutionOf<T::MinerConfig>>,
            &EPM::RawSolution<EPM::SolutionOf<T::MinerConfig>>,
        ),
        dpos_score: ElectionScore,
        dpos_unbounded_score: ElectionScore,
        snapshot_metadata: SolutionOrSnapshotSize,
        snapshot_size: usize,
        snapshot_metadata_unbound: SolutionOrSnapshotSize,
        snapshot_size_unbound: usize,
        min_active_stake: BalanceOf<T>,
    ) -> Self
    where
        BlockNumberFor<T>: Into<u32>,
        BalanceOf<T>: Into<u128>,
    {
        let (phrag_min_stake, phrag_sum_stake, phrag_sum_stake_squared) = {
            let ElectionScore {
                minimal_stake,
                sum_stake,
                sum_stake_squared,
            } = phrag_solutions.0.score;
            (minimal_stake, sum_stake, sum_stake_squared)
        };

        let (phrag_mms_min_stake, phrag_mms_sum_stake, phrag_mms_sum_stake_squared) = {
            let ElectionScore {
                minimal_stake,
                sum_stake,
                sum_stake_squared,
            } = phrag_solutions.1.score;
            (minimal_stake, sum_stake, sum_stake_squared)
        };

        let SolutionOrSnapshotSize { voters, targets } = snapshot_metadata;
        let (voters_unbound, targets_unbound) = (
            snapshot_metadata_unbound.voters,
            snapshot_metadata_unbound.targets,
        );

        Self {
            block_number: block_number.into(),
            phrag_min_stake,
            phrag_sum_stake,
            phrag_sum_stake_squared,
            phrag_mms_min_stake,
            phrag_mms_sum_stake,
            phrag_mms_sum_stake_squared,
            dpos_min_stake: dpos_score.minimal_stake,
            dpos_sum_stake: dpos_score.sum_stake,
            dpos_sum_stake_squared: dpos_score.sum_stake_squared,
            dpos_unbound_min_stake: dpos_unbounded_score.minimal_stake,
            dpos_unbound_sum_stake: dpos_unbounded_score.sum_stake,
            dpos_unbound_sum_stake_squared: dpos_unbounded_score.sum_stake_squared,
            voters,
            targets,
            snapshot_size,
            voters_unbound,
            targets_unbound,
            snapshot_size_unbound,
            min_active_stake: min_active_stake.into(),
            _marker: PhantomData,
        }
    }
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

                let phrag_raw_solution = gadgets::mine_with::<Runtime>(&Solver::SeqPhragmen{iterations: 10}, ext, false)?;
                let phrag_mms_raw_solution = gadgets::mine_with::<Runtime>(&Solver::PhragMMS{iterations: 10}, ext, false)?;

                let dpos_score = gadgets::mine_dpos::<Runtime>(ext)?;

                let (snapshot_metadata_unbound, snapshot_size_unbound) = gadgets::calculate_and_store_unbounded_snapshot::<Runtime>(ext)?;
                let dpos_unbound_score = gadgets::mine_dpos::<Runtime>(ext)?;

                let csv_entry = ElectionEntryCSV::<Runtime>::new(
                    block_number,
                    (&phrag_raw_solution, &phrag_mms_raw_solution),
                    dpos_score,
                    dpos_unbound_score,
                    snapshot_metadata,
                    snapshot_size,
                    snapshot_metadata_unbound,
                    snapshot_size_unbound,
                    min_active_stake,
                );

                crate::write_csv(csv_entry, &output_path)?;

                Ok(())
            }
        }
    };
}

min_active_stake_for!(polkadot);
min_active_stake_for!(kusama);
min_active_stake_for!(westend);

election_analysis_for!(polkadot);
election_analysis_for!(kusama);
election_analysis_for!(westend);
