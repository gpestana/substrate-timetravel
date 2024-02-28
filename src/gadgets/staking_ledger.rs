use super::*;

use sp_staking::StakingAccount;
use Staking::{Bonded, Config, Ledger, Payee};

/// For each ledger:
/// * `Bonded<T>` and `Payee<T>` are set.
/// * stash in `Bonded<T>` is the same as in the ledger.
fn ledger_checks<T: Config>() -> Vec<AccountIdOf<T>> {
    let mut bad_stashes = vec![];

    for (controller, ledger) in Ledger::<T>::iter() {
        let stash = ledger.stash;

        if let Some(bonded_controller) = Bonded::<T>::get(&stash) {
            if controller != bonded_controller {
                log::error!(
                    target: LOG_TARGET,
                    "ledger's controller does not match bonded controller. stash: {:?} (controllers: {:?} != {:?})",
                    stash,
                    controller,
                    bonded_controller,
                );
                bad_stashes.push(stash);
            }
        } else {
            log::error!(
                target: LOG_TARGET,
                "ledger's controller does not have a bonded stash. {:?}",
                stash,
            );
            bad_stashes.push(stash);
        }
    }

    bad_stashes
}

fn bonded_checks<T: Config>() -> (
    Vec<(AccountIdOf<T>, AccountIdOf<T>)>,
    Vec<(AccountIdOf<T>, AccountIdOf<T>)>,
    Vec<AccountIdOf<T>>,
) {
    let mut none_ledgers = vec![];
    let mut inconsistent_ledgers = vec![];
    let mut ok_ledgers = vec![];

    for (stash, controller) in Bonded::<T>::iter() {
        let ledger = Ledger::<T>::get(&controller);

        if ledger.is_none() {
            none_ledgers.push((stash.clone(), controller));
            log::error!(
                "{:?} with bonded does not have a ledger associated with the controller",
                stash,
            );
        } else {
            let ledger = ledger.expect("exists; qed.");
            if ledger.stash != stash {
                inconsistent_ledgers.push((stash.clone(), ledger.stash.clone()));
                log::error!(target: LOG_TARGET, "stash in ledger does not match expected {} != {}", ledger.stash, stash);
            }
            ok_ledgers.push(stash);
        }
    }
    (none_ledgers, inconsistent_ledgers, ok_ledgers)
}

/// Staking ledger consistency checks.
pub(crate) fn staking_ledger_checks<T>(mut exts: Vec<Ext>) -> Result<(), anyhow::Error>
where
    T: EPM::Config + Staking::Config,
{
    assert!(exts.len() == 2, "expected to have len 2");

    // select parent and child block externalities.
    let mut ext0 = exts.split_off(1);
    let (mut ext_parent, mut ext_child) = {
        let mut ext = ext0.pop().unwrap();
        let mut ext_other = exts.pop().unwrap();

        let bn1 = block_number::<T>(&mut ext);
        let bn2 = block_number::<T>(&mut ext_other);

        match bn1 > bn2 {
            true => (ext_other, ext),
            false => (ext, ext_other),
        }
    };

    let mut bad_ledgers = vec![];
    let mut none_ledgers = vec![];
    let mut inconsistent_ledgers = vec![];
    let mut ok_ledgers = vec![];

    // 1. process child first to obtain the faulty ledgers and generate report.
    let bn = block_number::<T>(&mut ext_child);
    ext_child.execute_with(|| {
        log::info!(target: LOG_TARGET, " ------ Running logic for child block #{:?}..", bn);

        let ledgers = Ledger::<T>::iter().count();
        let bonded = Bonded::<T>::iter().count();
        let payees = Payee::<T>::iter().count();

        log::info!(
            target: LOG_TARGET,
            "#ledgers: {}, #bonded: {}, #payees: {}",
            ledgers, bonded, payees,
        );

        // make sure #s are the same.
        if ledgers != payees || ledgers != bonded {
            log::error!(
                target: LOG_TARGET,
                "#s out of sync: #ledgers: {}, #bonded: {}, #payees: {}",
                ledgers, bonded, payees,
            );
        }

        bad_ledgers = ledger_checks::<T>();
        (none_ledgers, inconsistent_ledgers, ok_ledgers) = bonded_checks::<T>();
    });

    log::warn!(
        target: LOG_TARGET,
        " Report: # none_ledgers: {}; #consistent_ledgers {}, #bad_ledgers: {}",
        none_ledgers.len(),
        inconsistent_ledgers.len(),
        bad_ledgers.len(),
    );

    // 2, check parent block state of faulty ledgers.
    let bn = block_number::<T>(&mut ext_parent);
    ext_parent.execute_with(|| {
        log::info!(target: LOG_TARGET, " ------ Running logic for parent block #{:?}..", bn);

        log::info!(
            target: LOG_TARGET,
            "#ledgers: {}, #bonded: {}, #payees: {}",
            Ledger::<T>::iter().count(),
            Bonded::<T>::iter().count(),
            Payee::<T>::iter().count(),
        );

        // check if size of none_ledgers is the same as iterating over all staking ledgers and
        // check if their bonded stash is the same as the ledger stash.
        let inconsistent_ledgers = ledger_checks::<T>();
        log::warn!(
            target: LOG_TARGET,
            " Report: none_ledger: {:?}, inconsistent_ledgers: {:?}, ok_ledgers: {:?}, total_ledgers: {:?}",
            none_ledgers.len(),
            inconsistent_ledgers.len(),
            ok_ledgers.len(),
            Ledger::<T>::iter().count(),
        );

        // -- simulate deprecate_controller of faulty ledgers.
        deprecate_controller_simulation::<T>(none_ledgers);

        let ledgers = Ledger::<T>::iter().count();
        let bonded = Bonded::<T>::iter().count();
        let payees = Payee::<T>::iter().count();

        log::info!(
            target: LOG_TARGET,
            "After deprecate: #ledgers: {}, #bonded: {}, #payees: {}",
            ledgers, bonded, payees,
        );
    });

    Ok(())
}

fn deprecate_controller_simulation<T: Config>(batch: Vec<(AccountIdOf<T>, AccountIdOf<T>)>) {
    for (stash, controller) in batch {
        let ledger = <Staking::Pallet<T>>::ledger(StakingAccount::Controller(controller.clone()))
            .expect("ledger should exist for controller");
        let ledger_stash = ledger.stash.clone();

        if stash != ledger_stash {
            log::warn!(target: LOG_TARGET,
                "ledger stash != stash in batch {:?} {:?}",
                ledger_stash,
                stash,
            );
        }

        Bonded::<T>::insert(&stash, &stash);
        Ledger::<T>::remove(controller);
        Ledger::<T>::insert(stash, ledger);
    }
}
