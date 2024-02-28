//! CLI configs for `subtrate-timetravel`.

use super::*;
use crate::operations::Operation;

use clap::Parser;

use sp_core::H256;

/// Clap CLI ops.
#[derive(Debug, Clone, Parser)]
#[cfg_attr(test, derive(PartialEq))]
#[command(author, version, about)]
pub(crate) struct Opt {
    /// The `ws` node to connect to.
    #[arg(long, short, default_value = DEFAULT_URI, env = "URI", global = true)]
    pub uri: String,

    /// WS connection timeout in number of seconds.
    #[arg(long, default_value_t = 60)]
    pub connection_timeout: usize,

    /// WS request timeout in number of seconds.
    #[arg(long, default_value_t = 60 * 10)]
    pub request_timeout: usize,

    /// Externalities snapshot path to use.
    #[arg(long, short, default_value = "./", env = "SNAPSHOT_PATH")]
    pub snapshot_path: String,

    /// File path where to store the output of a tranform operation.
    #[arg(
        long,
        short,
        default_value = "./output.csv",
        env = "OUTPUT_PATH",
        global = true
    )]
    pub output_path: String,

    #[command(subcommand)]
    pub command: Command,
}

/// Commands for `substrate-etc` CLI.
#[derive(Debug, Clone, Parser)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum Command {
    /// Extracts keys from a remote RPC node, builds the externatilies and saves the snapshot to
    /// disk for later processing.
    Extract(ExtractConfig),

    /// Loads externality snapshot from disk and applies some operation over the storage items.
    Transform(TransformConfig),
}

/// Configs for the `extract` operation.
#[derive(Debug, Clone, Parser)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) struct ExtractConfig {
    /// The block hash at which scraping happens. If none is provided, the latest head is used.
    #[arg(long, env = "BN")]
    pub bn: Option<Vec<H256>>,

    /// List of pallets to scrap keys from the remote node and store in the snapshot.
    #[arg(long, env = "PALLETS", default_values_t = ["ElectionProviderMultiPhase".to_string(), "Staking".to_string(), "VoterList".to_string()])]
    pub pallets: Vec<String>,
}

/// Configs for the `transform` operation.
#[derive(Debug, Clone, Parser)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) struct TransformConfig {
    /// The block(s) hash(es) at which scraping happens. If none is provided, the latest head is used.
    #[arg(long, env = "BN")]
    pub bn: Option<Vec<H256>>,

    /// Compute unbounded election operations or not.
    #[arg(long, default_value_t = false)]
    pub compute_unbounded: bool,

    /// If run is live, then the snapshot is noe required and the remote externalities are created on the fly.
    #[arg(long, default_value_t = false)]
    pub live: bool,

    /// The operation to perform.
    #[command(subcommand)]
    pub operation: Operation,
}

/// Solvers for NPoS elections.
#[derive(Debug, Clone, Parser)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum Solver {
    SeqPhragmen {
        #[arg(long, default_value_t = 10)]
        iterations: usize,
    },
    PhragMMS {
        #[arg(long, default_value_t = 10)]
        iterations: usize,
    },
}
