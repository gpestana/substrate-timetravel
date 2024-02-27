//! # `substrate-timetravel` CLI
//!
//! #### A CLI tool to extract, process and load historical state from Substrate-based chains
//!
//! `substrate-timetravel` helps scrapping storage keys from remote substrate nodes and populate a
//! local externalities that can easily be turned into snapshots for ergonomics and fast
//! experimentation. It also provides an easy way to mutate and transform the state of
//! externalities using pre-developed `gadgets` that can be assembled into `operations`.
//!
//! Dividing the "extract" and "transform" phases offers an ergonomic way to analyse and tweak
//! historical chain data locally.
//!
//! The module `crate::gadgets` implements modular actions that a dev may find useful when
//! inspecting and interacting with a populated externalities. The module `crate::operations`
//! implements operations that use a set of gadgets to achieve a goal. For example, the
//! `election_analysis` computes, among other things, election scores using different election
//! algorithms and computes unbounded election snapshots given the state of the chain at a
//! particular block. Those computations rely on gadgets that are modular and generic to be used by
//! other operations.
//!
//! ## How to use the CLI
//!
//! #### 1. `substrate-timetravel extract`: Extract and store block state locally
//!
//! ```bash
//!  $ substrate-elt extract --at=<block_hash> --snapshot_path=<path> --pallets=Staking --uri=wss://rpc.polkadot.io:433
//! ```
//! This command will fetch the block keys from a remote node, build an externalities and store its
//! snapshot to disk for posterior analysis.
//!
//! For more information and configuration options, check `substrate-timetravel extract help`.
//!
//! #### 2. `substrate-timetravel transform`: Perform a transformation on a block state
//!
//! ```bash
//!  $ substrate-elt transform --at=<block_hash> min_active_stake --snapshot_path=<path> --uri=wss://rpc.polkadot.io:433
//! ```
//! The `min_active_stake` operation will calculate the minimum active stake of a block which
//! externalities snapshot has been stored under the snapshot_path.
//!
//! The advantage of splitting the `extract` from the `tranform` command is that several operations
//! and iterations can be applied over a stored externalities snapshot without having to constantly
//! download the block storage keys from a remote node.
//!
//! The output of the operation is written in the for of a CSV file in the `output_path` (set by
//! default as `./output.csv`).
//!
//! For more information and configuration options, check `substrate-timetravel extract help`.
//!
//! #### 3.Extract and transform in one command
//!
//! It is possible to collapse the `extract` and `transform` into one, which is specially helpful
//! for 1-time operations when the externalities snapshot does not yet exist. This can be achieved
//! by using the `--live` flag with the transform command:
//! ```bash
//!  $ substrate-elt transform --live --at=<block_hash> min_active_stake --snapshot_path=<path> --uri=wss://rpc.polkadot.io:433
//! ```
//!
//! The command above will 1) populate and store a remote externalities from a remote node and
//! 2) perform the `min_active_stake` operation over that state.
//!
//! ## Examples
//!
//! #### Fetch the minimum active stake from block
//!
//! ```bash
//!  $ cargo build
//!  $ RUST_LOG=info ./target/debug/substrate-timetravel transform --live --at=0x1477d54ad233824dd60afe1efc76413523c2737fd0cbabe2271568f75f560c74 min-active-stake --uri=wss://rpc.polkadot.io:443
//! ````
//! The result of the operation is saved in `./output.csv` in the form of
//!
//! ```csv
//! block_number,min_active_stake
//! 14401871,9517000000
//! ```
//! You can continue to call transform min-active-stake and the results will be appended to the output file:
//!
//! ```csv
//! block_number,min_active_stake
//! 14401871,9517000000
//! 15380091,9517000000
//! 14401873,9517000000
//! ```

mod commands;
mod configs;
mod gadgets;
mod operations;
mod prelude;
mod rpc;
mod utils;

use configs::{Command, Opt};
use prelude::*;

use clap::Parser;
use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use rpc::{RpcApiClient, SharedRpcClient};
use serde::Serialize;
use std::{ops::Deref, sync::Arc, time::Duration};
use thiserror::Error;

/// Errors of the CLI.
#[derive(Error, Debug)]
pub(crate) enum Error {
    #[error("Externalities error {error:?}")]
    Externalities { error: String },
}

/// Selector for diferent runtimes.
pub(crate) enum AnyRuntime {
    Polkadot,
    Kusama,
    Westend,
}

pub(crate) static mut RUNTIME: AnyRuntime = AnyRuntime::Polkadot;

macro_rules! construct_runtime_prelude {
    ($runtime:ident) => {
        paste::paste! {
        pub(crate) mod [<$runtime _runtime_exports>] {
            pub(crate) use crate::prelude::*;
            pub(crate) use [<$runtime _runtime>]::{Block, Runtime};
            pub(crate) use crate::commands::[<extract_cmd_ $runtime>] as extract_cmd;
            pub(crate) use crate::commands::[<transform_cmd_ $runtime>] as transform_cmd;
        }}
    };
}

//construct_runtime_prelude!(polkadot);
//construct_runtime_prelude!(kusama);
construct_runtime_prelude!(westend);

#[macro_export]
macro_rules! any_runtime {
	($($code:tt)*) => {
		unsafe {
			match $crate::RUNTIME {
				//$crate::AnyRuntime::Polkadot => {
				//	#[allow(unused)]
				// use $crate::polkadot_runtime_exports::*;
				//	$($code)*
				//},
				//$crate::AnyRuntime::Kusama => {
				//	#[allow(unused)]
				// use $crate::kusama_runtime_exports::*;
				//	$($code)*
				//},
				$crate::AnyRuntime::Westend => {
					#[allow(unused)]
					use $crate::westend_runtime_exports::*;
					$($code)*
				},
                _ => {
                	#[allow(unused)]
					use $crate::westend_runtime_exports::*;
					$($code)*
                },
			}
		}
	}
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let Opt {
        uri,
        command,
        connection_timeout,
        request_timeout,
        snapshot_path,
        output_path,
    } = Opt::parse();

    let rpc = loop {
        match SharedRpcClient::new(
            &uri,
            Duration::from_secs(connection_timeout as u64),
            Duration::from_secs(request_timeout as u64),
        )
        .await
        {
            Ok(client) => break client,
            Err(why) => {
                log::warn!(
                    target: LOG_TARGET,
                    "failed to connect to client due to {:?}, retrying soon..",
                    why
                );
                tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
            }
        }
    };

    let chain: String = rpc
        .system_chain()
        .await
        .expect("system_chain infallible; qed.");
    match chain.to_lowercase().as_str() {
        "polkadot" | "development" => {
            sp_core::crypto::set_default_ss58_version(
                sp_core::crypto::Ss58AddressFormatRegistry::PolkadotAccount.into(),
            );
            sub_tokens::dynamic::set_name("DOT");
            sub_tokens::dynamic::set_decimal_points(10_000_000_000);
            // safety: this program will always be single threaded, thus accessing global static is
            // safe.
            unsafe {
                RUNTIME = AnyRuntime::Polkadot;
            }
        }
        "kusama" | "kusama-dev" => {
            sp_core::crypto::set_default_ss58_version(
                sp_core::crypto::Ss58AddressFormatRegistry::KusamaAccount.into(),
            );
            sub_tokens::dynamic::set_name("KSM");
            sub_tokens::dynamic::set_decimal_points(1_000_000_000_000);
            // safety: this program will always be single threaded, thus accessing global static is
            // safe.
            unsafe {
                RUNTIME = AnyRuntime::Kusama;
            }
        }
        "westend" => {
            sp_core::crypto::set_default_ss58_version(
                sp_core::crypto::Ss58AddressFormatRegistry::PolkadotAccount.into(),
            );
            sub_tokens::dynamic::set_name("WND");
            sub_tokens::dynamic::set_decimal_points(1_000_000_000_000);
            // safety: this program will always be single threaded, thus accessing global static is
            // safe.
            unsafe {
                RUNTIME = AnyRuntime::Westend;
            }
        }
        _ => {
            eprintln!("unexpected chain: {:?}", chain);
            return;
        }
    }
    log::info!(target: LOG_TARGET, "connected to chain {:?}", chain);

    let outcome = any_runtime! {
        match command {
            Command::Extract(config) => {
                let block_hash = match config.at {
                    Some(bh) => bh,
                    None => {
                        log::error!(target: LOG_TARGET, "Config: expected a valid block hash (--at).");
                        return;
                    }
                };
                let file_path = format!("{}/{}.data", snapshot_path, block_hash);
                extract_cmd(rpc.uri().to_string(), config.pallets, block_hash, file_path, false).await
                .map_err(|e| {
                    log::error!(target: LOG_TARGET, "Extract error: {:?}", e);
                }).unwrap();
            },
            Command::Transform(config) => {
                let block_hash = match config.at {
                    Some(bh) => bh,
                    None => {
                        log::error!(target: LOG_TARGET, "Config: expected a valid block hash (--at).");
                        return;
                    }
                };
                let snapshot_path = format!("{}/{}.data", snapshot_path, block_hash);
                transform_cmd(
                    rpc.uri().to_string(),
                    config.operation,
                    block_hash,
                    output_path,
                    snapshot_path,
                    config.compute_unbounded,
                    config.live
                ).await
                .map_err(|e| {
                    log::error!(target: LOG_TARGET, "Transform error: {:?}", e);
                }).unwrap();
            },
        };
    };

    log::info!(
        target: LOG_TARGET,
        "round of execution finished. outcome = {:?}",
        outcome
    );
}

pub(crate) fn write_csv<E: Serialize>(entry: E, output_path: &str) -> Result<(), anyhow::Error> {
    let headers = if std::path::Path::new(output_path).exists() {
        false
    } else {
        true
    };
    let csv = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(output_path)?;

    let mut buffer = csv::WriterBuilder::new()
        .has_headers(headers)
        .from_writer(csv);
    buffer.serialize(entry)?;
    buffer.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::traits::Get;

    fn get_version<T: frame_system::Config>() -> sp_version::RuntimeVersion {
        T::Version::get()
    }

    #[test]
    fn any_runtime_works() {
        unsafe {
            RUNTIME = AnyRuntime::Polkadot;
        }
        let polkadot_version = any_runtime! { get_version::<Runtime>() };

        unsafe {
            RUNTIME = AnyRuntime::Kusama;
        }
        let kusama_version = any_runtime! { get_version::<Runtime>() };

        assert_eq!(polkadot_version.spec_name, "polkadot".into());
        assert_eq!(kusama_version.spec_name, "kusama".into());
    }
}
