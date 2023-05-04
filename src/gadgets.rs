use crate::prelude::*;

use codec::Encode;
use frame_system::pallet_prelude::BlockNumberFor;
use sp_npos_elections::{BalancingConfig, ElectionScore, EvaluateSupport};
use sp_runtime::traits::Zero;
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

pub(crate) fn min_active_stake<T: EPM::Config + Staking::Config>(ext: &mut Ext) -> BalanceOf<T>
where
    BalanceOf<T>: From<u64>,
{
    use frame_election_provider_support::SortedListProvider;
    const NPOS_MAX_ITERATIONS_COEFFICIENT: u32 = 2;

    ext.execute_with(|| {
        let weight_of = pallet_staking::Pallet::<T>::weight_of_fn();

        let maybe_max_len = None; // get this from somewhere.

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
