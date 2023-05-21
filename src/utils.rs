use std::{collections::BTreeMap, fmt::Debug};

#[derive(Copy, Clone, Debug)]
pub(crate) enum ShareDistribution {
    ProRata,
    Pareto,
}

#[derive(Debug, Clone)]
pub(crate) struct SortedTargets<A: Ord + Debug>(Vec<A>);

impl<A: Ord + Clone + Debug> SortedTargets<A> {
    pub fn from_voters<I>(voters: Vec<(A, u64, I)>) -> Self
    where
        I: IntoIterator<Item = A>,
    {
        let mut map = BTreeMap::new();

        for vote in voters.into_iter() {
            for target in vote.2.into_iter() {
                *map.entry(target).or_insert(0) += vote.1;
            }
        }

        let mut sorted_keys: Vec<A> = map.clone().into_iter().map(|(key, _)| key).collect();
        sorted_keys.sort_by_key(|key| map.get(key));

        Self(sorted_keys)
    }
}

pub(crate) fn share_distribution<A: Ord + Debug + Clone>(
    sorted_targets: &SortedTargets<A>,
    weight: u64,
    distribution: ShareDistribution,
) -> Vec<(A, u64)> {
    match distribution {
        ShareDistribution::ProRata => {
            let mut share_distribution = vec![];
            let share = weight / sorted_targets.0.len() as u64;
            for target in sorted_targets.0.clone().into_iter() {
                share_distribution.push((target, share));
            }

            share_distribution
        }
        ShareDistribution::Pareto => {
            // assumes `sorted_targets` is indeed sorted.
            let mut share_distribution = vec![];

            let split_index = (sorted_targets.0.len() as f32 * 0.8) as usize;
            let (bottom_eighty, top_twenty) = sorted_targets.0.split_at(split_index);

            let twenty_total_share = (weight as f32 * 0.2) as u64;
            let twenty_share = twenty_total_share / bottom_eighty.len() as u64;

            let eighty_total_share = (weight as f32 * 0.8) as u64;
            let eighty_share = eighty_total_share / top_twenty.len() as u64;

            // bottom 80% get 20% of the share.
            for target in bottom_eighty.into_iter() {
                share_distribution.push((target.clone(), twenty_share));
            }

            // top 20% get 80% of the share.
            for target in top_twenty.into_iter() {
                share_distribution.push((target.clone(), eighty_share));
            }

            share_distribution
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_votes_works() {
        let v: Vec<(u32, u64, Vec<u32>)> = vec![
            (1, 20, vec![1, 2]),
            (2, 10, vec![3]),
            (3, 10, vec![1, 3]),
            (4, 10, vec![4, 3]),
            (5, 10, vec![1, 3]),
        ];

        let sorted_targets = SortedTargets::<_>::from_voters(v);
        assert_eq!(sorted_targets.0, vec![4, 2, 1, 3]);
    }
    #[test]
    fn distributions_work() {
        let v: Vec<(u32, u64, Vec<u32>)> = vec![
            (1, 20, vec![1, 2]),
            (2, 10, vec![3]),
            (3, 10, vec![1, 3]),
            (4, 10, vec![4, 3]),
            (5, 10, vec![1, 3]),
        ];

        let sorted_targets = SortedTargets::<_>::from_voters(v);

        let prorata_distribution =
            share_distribution::<u32>(&sorted_targets, 100, ShareDistribution::ProRata);
        let pareto_distribution =
            share_distribution::<u32>(&sorted_targets, 100, ShareDistribution::Pareto);

        assert_eq!(
            prorata_distribution,
            vec![(4, 25), (2, 25), (1, 25), (3, 25)]
        );
        assert_eq!(pareto_distribution, vec![(4, 6), (2, 6), (1, 6), (3, 80)]);
    }
}
