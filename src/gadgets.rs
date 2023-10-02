//! Library of gadgets to apply over a given externalities.
//!
//! Gadgets are methods that extract and mutate runtime state based on a given externalities. The
//! gadgets are built to be modular and used across operations.

use crate::configs::Solver;
use crate::prelude::*;
use crate::utils;

use anyhow::anyhow;

use codec::Encode;
use frame_election_provider_support::NposSolver;
use frame_election_provider_support::{
    ElectionDataProvider, ElectionProviderBase, NposSolution, SortedListProvider,
};
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_npos_elections::{BalancingConfig, ElectionScore, EvaluateSupport};
use sp_runtime::{traits::Zero, SaturatedConversion};
use Staking::{ActiveEraInfo, BalanceOf as BalanceOfS};
use EPM::{BalanceOf, RoundSnapshot, SolutionOrSnapshotSize};

/// Returns the current block number.
pub(crate) fn block_number<T: EPM::Config>(ext: &mut Ext) -> BlockNumberFor<T> {
    ext.execute_with(|| <frame_system::Pallet<T>>::block_number())
}

/// Returns the current active era.
pub(crate) fn active_era<T: Staking::Config>(ext: &mut Ext) -> Option<ActiveEraInfo> {
    ext.execute_with(|| <Staking::ActiveEra<T>>::get())
}

/// Returns the snapshot bounds and encoded size.
///
/// If the snapshot does not exist in the current externalities, it creates a new one using the
/// same algorithm as the runtime.
pub(crate) fn snapshot_data_or_force<T: EPM::Config>(
    ext: &mut Ext,
) -> (SolutionOrSnapshotSize, usize) {
    ext.execute_with(|| {
        if <EPM::Snapshot<T>>::get().is_some() {
            log::info!(
                target: LOG_TARGET,
                "snapshot_data_or_force: snapshot already exists."
            );
        } else {
            log::info!(
                target: LOG_TARGET,
                "snapshot_data_or_force: creating a snapshot now."
            );
            <EPM::Pallet<T>>::create_snapshot().unwrap();
        };

        (
            <EPM::SnapshotMetadata<T>>::get().expect("snapshot metadata should exist by now. qed."),
            <EPM::Pallet<T>>::snapshot()
                .expect("snapshot should exist by now. qed.")
                .encode()
                .len(),
        )
    })
}

/// Computes a new unbounded snapshot and stores it.
///
/// The new snapshot is unbounded in terms of the number of voters, i.e., all the voters in the
/// voter list will be used in the creation of the new snashot. The target bound remains
/// `MaxElectableTargets`.
pub(crate) fn compute_and_store_unbounded_snapshot<T>(
    ext: &mut Ext,
) -> Result<(SolutionOrSnapshotSize, usize), anyhow::Error>
where
    T: EPM::Config + Staking::Config,
    EPM::Pallet<T>: ElectionProviderBase,
{
    ext.execute_with(|| {
        EPM::Pallet::<T>::kill_snapshot();
        assert!(<EPM::Snapshot<T>>::get().is_none());

        let target_limit = <T::MaxElectableTargets>::get().saturated_into::<usize>();
        let voter_limit = <<T as Staking::Config>::VoterList>::iter().count();

        let targets =
            <<T as EPM::Config>::DataProvider as ElectionDataProvider>::electable_targets(Some(
                target_limit,
            ))
            .map_err(|e| anyhow!(e.to_string()))?;

        let voters = <<T as EPM::Config>::DataProvider as ElectionDataProvider>::electing_voters(
            Some(voter_limit),
        )
        .map_err(|e| anyhow!(e.to_string()))?;

        let mut desired_targets =
            <EPM::Pallet<T> as ElectionProviderBase>::desired_targets_checked()
                .map_err(|e| anyhow!(e.to_string()))?;
        let max_desired_targets: u32 = targets.len() as u32;
        if desired_targets > max_desired_targets {
            log::warn!(
                target: LOG_TARGET,
                "desired_targets: {} > targets.len(): {}, capping desired_targets",
                desired_targets,
                max_desired_targets
            );
            desired_targets = max_desired_targets;
        }

        // mimic `EPM::create_snashot_internal` and store voter-unbounded snapshot.
        let metadata = SolutionOrSnapshotSize {
            voters: voters.len() as u32,
            targets: targets.len() as u32,
        };

        let snapshot = RoundSnapshot::<_, _> { voters, targets };
        <EPM::Snapshot<T>>::put(snapshot);
        <EPM::SnapshotMetadata<T>>::put(metadata);
        <EPM::DesiredTargets<T>>::put(desired_targets);

        // pull from storage to ensure snapshot is set.
        let snapshot_len = <EPM::Snapshot<T>>::get()
            .expect("snapshot should exist, qed.")
            .encode()
            .len();

        Ok((metadata, snapshot_len))
    })
}

/// Calculates the era_payout in the current block.
pub(crate) fn era_payout<T>(ext: &mut Ext) -> (BalanceOfS<T>, BalanceOfS<T>)
where
    T: Timestamp::Config + Staking::Config,
    BalanceOfS<T>: From<u64>,
{
    log::info!(target: LOG_TARGET, "Calculating era_payout.");

    ext.execute_with(|| (10.into(), 20.into()))
}

/// Calculates the minimum active stake for a existing snapshot.
pub(crate) fn min_active_stake<T: EPM::Config + Staking::Config>(ext: &mut Ext) -> BalanceOf<T>
where
    BalanceOf<T>: From<u64>,
{
    log::info!(target: LOG_TARGET, "Calculating min_active_state.");

    //use frame_election_provider_support::SortedListProvider;
    const NPOS_MAX_ITERATIONS_COEFFICIENT: u32 = 2;

    ext.execute_with(|| {
        let weight_of = pallet_staking::Pallet::<T>::weight_of_fn();
        let maybe_max_len = Some(T::MaxElectingVoters::get().saturated_into::<usize>());

        let max_allowed_len = {
            let all_voter_count = T::VoterList::count() as usize;
            maybe_max_len
                .unwrap_or(all_voter_count)
                .min(all_voter_count)
        };

        let mut all_voters = Vec::<_>::with_capacity(max_allowed_len);
        let mut min_active_stake = u64::MAX;
        let mut voters_seen = 0u32;

        let mut sorted_voters = T::VoterList::iter();
        while all_voters.len() < max_allowed_len
            && voters_seen < (NPOS_MAX_ITERATIONS_COEFFICIENT * max_allowed_len as u32)
        {
            let voter = match sorted_voters.next() {
                Some(voter) => {
                    voters_seen += 1;
                    voter
                }
                None => break,
            };

            let voter_weight = weight_of(&voter);
            if voter_weight.is_zero() {
                continue;
            }

            min_active_stake = if voter_weight < min_active_stake {
                voter_weight
            } else {
                min_active_stake
            };

            // it doesn't really matter here.
            all_voters.push(min_active_stake);
        }
        min_active_stake.into()
    })
}

/// Compute the election. It expects to NOT be `Phase::Off`. In other words, the snapshot must
/// exists on the given externalities.
fn mine_solution<T, S>(
    ext: &mut Ext,
    do_feasibility: bool,
) -> Result<EPM::RawSolution<EPM::SolutionOf<T::MinerConfig>>, anyhow::Error>
where
    T: EPM::Config,
    S: NposSolver<
        Error = <<T as EPM::Config>::Solver as NposSolver>::Error,
        AccountId = <<T as EPM::Config>::Solver as NposSolver>::AccountId,
    >,
{
    ext.execute_with(|| {
        let (raw_solution, _) = <EPM::Pallet<T>>::mine_solution()
            .map_err(|e| anyhow!("Error mining solution: {:?}.", e))?;
        if do_feasibility {
            let _ = <EPM::Pallet<T>>::feasibility_check(
                raw_solution.clone(),
                EPM::ElectionCompute::Signed,
            )
            .map_err(|e| anyhow!("Error calculating feasibility check: {:?}.", e))?;
        }

        let voter_count = raw_solution.solution.voter_count();
        let target_count = raw_solution.solution.unique_targets().len();

        log::info!(
            target: LOG_TARGET,
            "mined a npos-like solution (voters: {:?}, targets: {:?}).",
            voter_count,
            target_count,
        );

        Ok(raw_solution)
    })
}

frame_support::parameter_types! {
    /// Number of balancing iterations for a solution algorithm. Set based on the [`Solvers`] CLI
    /// config.
    pub static BalanceIterations: usize = 10;
    pub static Balancing: Option<BalancingConfig> = Some( BalancingConfig { iterations: BalanceIterations::get(), tolerance: 0 } );
}

/// Mines an election solution given a solver.
pub(crate) fn mine_with<T>(
    solver: &Solver,
    ext: &mut Ext,
    do_feasibility: bool,
) -> Result<EPM::RawSolution<EPM::SolutionOf<T::MinerConfig>>, anyhow::Error>
where
    T: EPM::Config,
    T::Solver: NposSolver<Error = sp_npos_elections::Error>,
{
    log::info!(target: LOG_TARGET, "Mining NPoS.");

    use frame_election_provider_support::{PhragMMS, SequentialPhragmen};

    match solver {
        Solver::SeqPhragmen { iterations } => {
            BalanceIterations::set(*iterations);
            mine_solution::<
                T,
                SequentialPhragmen<
                    <T as frame_system::Config>::AccountId,
                    sp_runtime::Perbill,
                    Balancing,
                >,
            >(ext, do_feasibility)
        }
        Solver::PhragMMS { iterations } => {
            BalanceIterations::set(*iterations);
            mine_solution::<
                T,
                PhragMMS<<T as frame_system::Config>::AccountId, sp_runtime::Perbill, Balancing>,
            >(ext, do_feasibility)
        }
    }
}

/// Mines a Delegated Proof-of-Stake (DPoS) given the current snapshot and returns the election
/// score.
///
/// In this DPoS flavour, the vote weight (stake) of the nominators' votes are distributed equaly
/// across their targets. The number of voters considered for the election is defined by the
/// snapshot state. The number of final winners is defined by `EPM::DesiredTargets`.
pub(crate) fn mine_dpos<T>(
    ext: &mut Ext,
    distribution_type: utils::ShareDistribution,
) -> Result<ElectionScore, anyhow::Error>
where
    T: EPM::Config + Staking::Config,
{
    ext.execute_with(|| {
        log::info!(target: LOG_TARGET, "Mining DPoS with {:?}.", distribution_type);

        let RoundSnapshot { voters, targets } =
            EPM::Snapshot::<T>::get().ok_or(anyhow!("Snapshot did not exist."))?;
        let snapshot_targets = targets;
        let desired_targets =
            EPM::DesiredTargets::<T>::get().ok_or(anyhow!("Desired targets did not exist."))?;

        let skip_targets = 0;
        let mut num_votes_per_voter = vec![];
        let mut assignments: Vec<sp_npos_elections::StakedAssignment<T::AccountId>> = vec![];

        let sorted_targets_by_stake = utils::SortedTargets::<_>::from_voters(voters.clone());

        voters.into_iter().for_each(|(who, stake, targets)| {
            if targets.is_empty() || stake == 0 {
                log::warn!(
                    target: LOG_TARGET,
                    "Bad voter with stake {:?}, targets: {:?}. skipping.",
                    stake,
                    targets.len()
                );
                return;
            }

            num_votes_per_voter.push(targets.len());

            let mut distribution = vec![];
            let shares = utils::share_distribution::<T::AccountId>(&sorted_targets_by_stake, stake, distribution_type);
            for share in shares {
                //if !<<T as Staking::Config>::TargetList as SortedListProvider<AccountIdOf<T>>>::contains(&share.0) {
                //    skip_targets = skip_targets + 1;
                //} else {
                distribution.push((share.0, share.1 as u128));
                //}
            }
            assignments.push(sp_npos_elections::StakedAssignment { who, distribution });
        });

        let mut supports = Vec::from_iter(sp_npos_elections::to_supports(&assignments[..]));
        let supports_len = supports.len();
        supports.sort_by_key(|(_, support)| support.total);
        let supports = supports
            .into_iter()
            .rev()
            .take(desired_targets as usize)
            .collect::<Vec<_>>();
        let supports_sorted = sp_npos_elections::Supports::from(supports);

        let score = supports_sorted.evaluate();

        log::info!(
            target: LOG_TARGET,
            "mined a dpos-like solution with score = {:?}. Targets with votes: {} (from which, {} desired winners were selected). Skipped {} targets from: snapshot {}, target_list: {}. Avg votes per voter: {}.",
            score,
            supports_len,
            desired_targets,
            skip_targets,
            snapshot_targets.len(),
            <<T as Staking::Config>::TargetList as SortedListProvider<AccountIdOf<T>>>::iter().count(),
            num_votes_per_voter.iter().sum::<usize>() as f32 / num_votes_per_voter.len() as f32,
        );

        Ok(score)
    })
}
