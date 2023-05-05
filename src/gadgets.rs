use crate::configs::Solver;
use crate::prelude::*;
use crate::Error;

use anyhow::anyhow;

use codec::Encode;
use frame_election_provider_support::NposSolver;
use frame_election_provider_support::{ElectionDataProvider, SortedListProvider};
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_npos_elections::{BalancingConfig, ElectionScore, EvaluateSupport};
use sp_runtime::{traits::Zero, SaturatedConversion};
use EPM::{BalanceOf, RoundSnapshot, SolutionOrSnapshotSize};

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
                "snapshot_data_or_force: creating a fake snapshot now."
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

pub(crate) fn calculate_and_store_unbounded_snapshot<T>(
    ext: &mut Ext,
) -> Result<(SolutionOrSnapshotSize, usize), anyhow::Error>
where
    T: EPM::Config + Staking::Config,
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
            .map_err(|e| {
                return anyhow!(Error::RuntimeError {
                    error: e.to_string()
                });
            })?;

        let voters = <<T as EPM::Config>::DataProvider as ElectionDataProvider>::electing_voters(
            Some(voter_limit),
        )
        .map_err(|e| {
            return anyhow!(Error::RuntimeError {
                error: e.to_string()
            });
        })?;

        // mimic `EPM::create_snashot_internal` and store voter-unbounded snapshot.
        let metadata = SolutionOrSnapshotSize {
            voters: voters.len() as u32,
            targets: targets.len() as u32,
        };
        <EPM::SnapshotMetadata<T>>::put(metadata);
        <EPM::DesiredTargets<T>>::put(target_limit as u32);

        let snapshot = RoundSnapshot::<_, _> { voters, targets };
        <EPM::Snapshot<T>>::put(snapshot);

        Ok((metadata, <EPM::Snapshot<T>>::get().unwrap().encode().len()))
    })
}

pub(crate) fn min_active_stake<T: EPM::Config + Staking::Config>(ext: &mut Ext) -> BalanceOf<T>
where
    BalanceOf<T>: From<u64>,
{
    use frame_election_provider_support::SortedListProvider;
    const NPOS_MAX_ITERATIONS_COEFFICIENT: u32 = 2;

    ext.execute_with(|| {
        let weight_of = pallet_staking::Pallet::<T>::weight_of_fn();

        let maybe_max_len = None; // TODO: parameterize fn and get this from somewhere.

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

pub(crate) fn block_number<T: EPM::Config>(ext: &mut Ext) -> BlockNumberFor<T> {
    ext.execute_with(|| <frame_system::Pallet<T>>::block_number())
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
        let (solution, _) = <EPM::Pallet<T>>::mine_solution().unwrap(); // TODO: throw anyhow
        if do_feasibility {
            let _ =
                <EPM::Pallet<T>>::feasibility_check(solution.clone(), EPM::ElectionCompute::Signed)
                    .unwrap(); // TODO:: throw anyhow
        }
        Ok(solution)
    })
}

frame_support::parameter_types! {
    /// Number of balancing iterations for a solution algorithm. Set based on the [`Solvers`] CLI
    /// config.
    pub static BalanceIterations: usize = 10;
    pub static Balancing: Option<BalancingConfig> = Some( BalancingConfig { iterations: BalanceIterations::get(), tolerance: 0 } );
}

pub(crate) fn mine_with<T>(
    solver: &Solver,
    ext: &mut Ext,
    do_feasibility: bool,
) -> Result<EPM::RawSolution<EPM::SolutionOf<T::MinerConfig>>, anyhow::Error>
where
    T: EPM::Config,
    T::Solver: NposSolver<Error = sp_npos_elections::Error>,
{
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

pub(crate) fn mine_dpos<T: EPM::Config>(ext: &mut Ext) -> Result<ElectionScore, anyhow::Error> {
    ext.execute_with(|| {
        use EPM::{RoundSnapshot, SolutionOrSnapshotSize};
        let RoundSnapshot { voters, .. } = EPM::Snapshot::<T>::get().unwrap();
        let desired_targets = EPM::DesiredTargets::<T>::get().unwrap();

        let mut assignments: Vec<sp_npos_elections::StakedAssignment<T::AccountId>> = vec![];

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

            let mut distribution = vec![];
            let share: u128 = (stake as u128) / (targets.len() as u128);
            for target in targets {
                distribution.push((target.clone(), share));
            }

            assignments.push(sp_npos_elections::StakedAssignment { who, distribution });
        });

        let mut supports = Vec::from_iter(sp_npos_elections::to_supports(&assignments[..]));
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
            "mined a dpos-like solution with score = {:?}",
            score
        );

        Ok(score)
    })
}
