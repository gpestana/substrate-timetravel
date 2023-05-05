// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! A CLI tool to extract, process and load Substrate chains state.

#![allow(unused)] // TODO: remove when stable

mod commands;
mod configs;
mod gadgets;
mod operations;
mod prelude;
mod rpc;

use configs::{Command, Opt};
use prelude::*;

use frame_support::traits::Get;

use anyhow::anyhow;
use clap::Parser;
use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use rpc::{RpcApiClient, SharedRpcClient};
use serde::Serialize;
use std::{ops::Deref, sync::Arc, time::Duration};
use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum Error {
    #[error("Config: expected a valid block hash")]
    ConfigAtMissing,

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
            pub(crate) use [<$runtime _runtime>]::*;
            pub(crate) use crate::commands::[<extract_cmd_ $runtime>] as extract_cmd;
            pub(crate) use crate::commands::[<transform_cmd_ $runtime>] as transform_cmd;
        }}
    };
}

construct_runtime_prelude!(polkadot);
construct_runtime_prelude!(kusama);
construct_runtime_prelude!(westend);

#[macro_export]
macro_rules! any_runtime {
	($($code:tt)*) => {
		unsafe {
			match $crate::RUNTIME {
				$crate::AnyRuntime::Polkadot => {
					#[allow(unused)]
					use $crate::polkadot_runtime_exports::*;
					$($code)*
				},
				$crate::AnyRuntime::Kusama => {
					#[allow(unused)]
					use $crate::kusama_runtime_exports::*;
					$($code)*
				},
				$crate::AnyRuntime::Westend => {
					#[allow(unused)]
					use $crate::westend_runtime_exports::*;
					$($code)*
				}
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
                extract_cmd(rpc.uri().to_string(), config.pallets, block_hash, file_path).await
                .map_err(|e| {
                    log::error!(target: LOG_TARGET, "Extract error: {:?}", e);
                });
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
                transform_cmd(rpc.uri().to_string(), config.operation, block_hash, output_path ,snapshot_path).await
                .map_err(|e| {
                    log::error!(target: LOG_TARGET, "Transform error: {:?}", e);
                });
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
